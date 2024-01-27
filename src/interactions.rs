use std::{
    collections::VecDeque,
    mem::MaybeUninit,
    time::{Duration, Instant},
};

use glam::{vec2, vec3, Affine3A, Vec2, Vec3};
use log::debug;
use stereokit::{
    ButtonState, Color32, CullMode, Handed, Pose, Ray, SkDraw, StereoKitDraw, StereoKitMultiThread,
    StereoKitSingleThread,
};

use crate::{
    overlay::{OverlayData, RelativeTo},
    AppSession,
};

const HANDS: [Handed; 2] = [Handed::Left, Handed::Right];

pub const HAND_LEFT: usize = 0;
pub const HAND_RIGHT: usize = 1;

pub const POINTER_NORM: u16 = 0;
pub const POINTER_SHIFT: u16 = 1;
pub const POINTER_ALT: u16 = 2;

pub trait InteractionHandler {
    fn on_hover(&mut self, hit: &PointerHit);
    fn on_left(&mut self, hand: usize);
    fn on_pointer(&mut self, session: &AppSession, hit: &PointerHit, pressed: bool);
    fn on_scroll(&mut self, hit: &PointerHit, delta: f32);
}

pub struct InputState {
    pub hmd: Affine3A,
    pointers: [PointerData; 2],
}

pub struct PointerData {
    hand: usize,
    release_actions: VecDeque<Box<dyn Fn()>>,
    now: PointerState,
    before: PointerState,
    mode: u16,
    colors: [Color32; 3],
    pose: Pose,
    pose3a: Affine3A,
    grabbed_offset: (Vec3, Vec3),
    grabbed_idx: Option<usize>,
    clicked_idx: Option<usize>,
    hovered_idx: Option<usize>,
    next_push: Instant,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerState {
    pressed: bool,
    grabbing: bool,
    show_hide: bool,
    scroll: f32,
}

pub struct PointerHit {
    pub hand: usize,
    pub mode: u16,
    pub primary: bool,
    pub uv: Vec2,
    pub dist: f32,
}

impl InputState {
    pub fn new(session: &AppSession) -> Self {
        Self {
            hmd: Affine3A::IDENTITY,
            pointers: [PointerData::new(session, 0), PointerData::new(session, 1)],
        }
    }

    pub fn update(&mut self, session: &AppSession, sk: &SkDraw, interactables: &mut [OverlayData]) {
        let hmd_pose = sk.input_head();
        self.hmd = Affine3A::from_rotation_translation(hmd_pose.orientation, hmd_pose.position);
        for h in 0..2 {
            self.pointers[h].update(session, &hmd_pose, sk);
        }

        for overlay in interactables.iter_mut() {
            match overlay.relative_to {
                RelativeTo::Head => {
                    let scale =
                        Affine3A::from_scale(vec3(overlay.width, overlay.width, overlay.width));
                    overlay.transform = self.hmd
                        * Affine3A::from_rotation_translation(
                            overlay.spawn_rotation,
                            overlay.spawn_point,
                        )
                        * scale;
                }
                RelativeTo::Hand(h) => {
                    let scale =
                        Affine3A::from_scale(vec3(overlay.width, overlay.width, overlay.width));
                    overlay.transform = self.pointers[h].pose3a
                        * Affine3A::from_rotation_translation(
                            overlay.spawn_rotation,
                            overlay.spawn_point,
                        )
                        * scale;
                }
                _ => {}
            }
        }

        for h in 0..2 {
            self.pointers[h].test_interactions(session, &self.hmd, sk, interactables);
        }
    }
}

impl PointerData {
    fn new(session: &AppSession, idx: usize) -> Self {
        PointerData {
            hand: session.primary_hand - idx,
            release_actions: VecDeque::new(),
            now: PointerState::default(),
            before: PointerState::default(),
            mode: 0,
            pose: Pose::IDENTITY,
            pose3a: Affine3A::IDENTITY,
            clicked_idx: None,
            grabbed_idx: None,
            grabbed_offset: (Vec3::ZERO, Vec3::ZERO),
            hovered_idx: None,
            colors: [session.color_norm, session.color_shift, session.color_alt],
            next_push: Instant::now(),
        }
    }

    fn update(&mut self, session: &AppSession, hmd: &Pose, sk: &SkDraw) {
        let controller = sk.input_controller(HANDS[self.hand]);

        self.pose = controller.aim;
        self.pose3a =
            Affine3A::from_rotation_translation(self.pose.orientation, self.pose.position);

        self.before = self.now;
        self.now.pressed = if self.before.pressed {
            controller.trigger >= (session.config.trigger_threshold - 0.1).max(0.0)
        } else {
            controller.trigger >= session.config.trigger_threshold
        };

        self.now.grabbing = if self.before.grabbing {
            controller.grip >= (session.config.grab_threshold - 0.05).max(0.0)
        } else {
            controller.grip >= session.config.grab_threshold
        };

        self.now.show_hide = if self.hand == 0 {
            sk.input_controller_menu() == ButtonState::ACTIVE
        } else {
            false
        };
        self.now.scroll = controller.stick.y;

        // If unpressed (true -> false)
        if self.before.pressed && !self.now.pressed {
            while let Some(action) = self.release_actions.pop_front() {
                action();
            }
        }

        let from_hmd = controller.palm.position - hmd.position;
        let dot = from_hmd.dot(controller.palm.forward());
        self.mode = if dot > 0.2 {
            POINTER_NORM // neutral
        } else {
            POINTER_SHIFT // palm down
        }
    }

    fn test_interactions(
        &mut self,
        session: &AppSession,
        hmd3a: &Affine3A,
        sk: &SkDraw,
        interactables: &mut [OverlayData],
    ) {
        let color = self.colors[self.mode as usize];

        // Grabbing an overlay
        if let Some(grabbed_idx) = self.grabbed_idx {
            let grabbed = &mut interactables[grabbed_idx];

            if grabbed.primary_pointer != Some(self.hand) {
                debug!("Pointer {}: Grab lost on {}", self.hand, grabbed.name);
                self.grabbed_idx = None;
                // ignore and continue
            } else if !self.now.grabbing {
                debug!("Pointer {}: Dropped {}", self.hand, grabbed.name);
                self.grabbed_idx = None;
                grabbed.on_drop();
                // drop and continue
            } else {
                if self.now.scroll.abs() > 0.1 {
                    if self.mode == POINTER_SHIFT {
                        if self.next_push < Instant::now() {
                            debug!("Pointer {}: Resize {}", self.hand, grabbed.name);
                            grabbed.on_size(self.now.scroll);
                            self.next_push = Instant::now() + Duration::from_millis(20);
                        }
                    } else if self.next_push < Instant::now() {
                        debug!("Pointer {}: Push/pull {}", self.hand, grabbed.name);
                        let offset = self.grabbed_offset.0
                            + self.grabbed_offset.0.normalize_or_zero() * self.now.scroll * 0.1;
                        let len_sq = offset.length_squared();
                        if len_sq > 0.20 && len_sq < 100. {
                            self.grabbed_offset.0 = offset;
                        }
                        self.next_push = Instant::now() + Duration::from_millis(20);
                    }
                }
                sk.hierarchy_push(self.pose3a);
                let grab_point = sk.hierarchy_to_world_point(self.grabbed_offset.0);
                grabbed.on_move(grab_point.into(), hmd3a);
                sk.hierarchy_pop();

                let mut points = vec![];
                sk.hierarchy_push(grabbed.transform);
                points.push(sk.hierarchy_to_world_point(vec3(-1., 0., 0.)));
                points.push(sk.hierarchy_to_world_point(vec3(1., 0., 0.)));
                sk.hierarchy_pop();

                for p in points.iter() {
                    sk.line_add(self.pose.position, *p, color, color, 0.002);
                }

                if self.now.pressed && !self.before.pressed {
                    debug!("Pointer {}: on_curve {}", self.hand, grabbed.name);
                    grabbed.on_curve();
                }
                return;
            }
        }

        // Test for new hits
        let mut hits: [RayHit; 8] = unsafe { MaybeUninit::zeroed().assume_init() };
        let mut num_hits = 0usize;

        for (idx, overlay) in interactables.iter_mut().enumerate() {
            if !overlay.visible {
                continue;
            }

            if let Some(gfx) = overlay.gfx.as_ref() {
                sk.hierarchy_push(overlay.transform);
                let ray = Ray::new(
                    sk.hierarchy_to_local_point(self.pose.position),
                    sk.hierarchy_to_local_direction(self.pose.forward()),
                );

                if let Some((hit, _)) = sk.mesh_ray_intersect(&gfx.mesh, ray, CullMode::Back) {
                    let ray_pos_worldspace = overlay.transform.transform_point3(ray.pos);
                    let hit_pos_worldspace = overlay.transform.transform_point3(hit.pos);

                    let vec = overlay.interaction_transform.transform_point3(hit.pos);
                    hits[num_hits] = RayHit {
                        idx,
                        ray_pos: ray.pos,
                        hit_pos: hit.pos,
                        uv: vec2(vec.x, vec.y),
                        dist: hit_pos_worldspace.distance(ray_pos_worldspace), /* Used to sort by distance in worldspace units */
                    };
                    num_hits += 1;
                    if num_hits > 7 {
                        sk.hierarchy_pop();
                        break;
                    }
                }
                sk.hierarchy_pop();
            }
        }

        if let Some(hit) = hits[..num_hits]
            .iter()
            .max_by(|a, b| b.dist.total_cmp(&a.dist))
        {
            let now_idx = hit.idx;
            let mut hit_data = PointerHit {
                hand: self.hand,
                mode: self.mode,
                uv: hit.uv,
                dist: hit.dist,
                primary: false,
            };

            // Invoke on_left
            if let Some(hovered_idx) = self.hovered_idx {
                if hovered_idx != now_idx {
                    let hovered = &mut interactables[hovered_idx];
                    if hovered.primary_pointer == Some(self.hand) {
                        hovered.primary_pointer = None;
                        debug!("Pointer {}: on_left {}", self.hand, hovered.name);
                        hovered.backend.on_left(self.hand);
                    }
                }
            }
            self.hovered_idx = Some(now_idx);

            let overlay = &mut interactables[now_idx];
            sk.hierarchy_push(overlay.transform);
            sk.line_add(hit.ray_pos, hit.hit_pos, color, color, 0.002);
            sk.hierarchy_pop();

            // grab start
            if self.now.grabbing && !self.before.grabbing && overlay.grabbable {
                overlay.primary_pointer = Some(self.hand);
                let mat =
                    Affine3A::from_rotation_translation(self.pose.orientation, self.pose.position);
                sk.hierarchy_push(mat);
                self.grabbed_offset.0 = sk.hierarchy_to_local_point(overlay.transform.translation);
                sk.hierarchy_pop();
                self.grabbed_idx = Some(now_idx);
                debug!("Pointer {}: Grabbed {}", self.hand, overlay.name);
                return;
            }

            // hover
            if let Some(primary_pointer) = overlay.primary_pointer {
                hit_data.primary = primary_pointer == self.hand;
            } else {
                overlay.primary_pointer = Some(self.hand);
                hit_data.primary = true;
            }

            overlay.backend.on_hover(&hit_data);

            if self.now.scroll.abs() > 0.1 {
                overlay.backend.on_scroll(&hit_data, self.now.scroll);
            }

            if self.now.pressed && !self.before.pressed {
                overlay.primary_pointer = Some(self.hand);
                hit_data.primary = true;
                self.clicked_idx = Some(now_idx);
                overlay.backend.on_pointer(session, &hit_data, true);
            } else if !self.now.pressed && self.before.pressed {
                if let Some(clicked_idx) = self.clicked_idx.take() {
                    let clicked = &mut interactables[clicked_idx];
                    clicked.backend.on_pointer(session, &hit_data, false);
                } else {
                    overlay.backend.on_pointer(session, &hit_data, false);
                }
            }
        } else {
            // no hit
            if let Some(idx) = self.hovered_idx {
                let obj = &mut interactables[idx];
                if obj.primary_pointer == Some(self.hand) {
                    obj.primary_pointer = None;
                }
                obj.backend.on_left(self.hand);
                self.hovered_idx = None;
            }

            if !self.now.pressed && self.before.pressed {
                if let Some(clicked_idx) = self.clicked_idx.take() {
                    let clicked = &mut interactables[clicked_idx];
                    clicked.backend.on_pointer(
                        session,
                        &PointerHit {
                            hand: self.hand,
                            mode: self.mode,
                            uv: vec2(0., 0.),
                            dist: 0.,
                            primary: true,
                        },
                        false,
                    );
                }
            }
        }
    }
}

struct RayHit {
    idx: usize,
    ray_pos: Vec3,
    hit_pos: Vec3,
    uv: Vec2,
    dist: f32,
}

// --- Dummies & plumbing below ---

impl Default for PointerState {
    fn default() -> Self {
        Self {
            pressed: false,
            grabbing: false,
            show_hide: false,
            scroll: 0.,
        }
    }
}

pub struct DummyInteractionHandler;

impl InteractionHandler for DummyInteractionHandler {
    fn on_left(&mut self, _hand: usize) {}
    fn on_hover(&mut self, _hit: &crate::interactions::PointerHit) {}
    fn on_pointer(
        &mut self,
        _session: &AppSession,
        _hit: &crate::interactions::PointerHit,
        _pressed: bool,
    ) {
    }
    fn on_scroll(&mut self, _hit: &crate::interactions::PointerHit, _delta: f32) {}
}
