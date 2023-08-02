use std::{collections::VecDeque, mem::MaybeUninit};

use glam::{vec2, Affine3A, Vec2, Vec3};
use log::debug;
use stereokit::{
    ButtonState, Color32, CullMode, Handed, Pose, Ray, SkDraw, StereoKitDraw, StereoKitMultiThread,
    StereoKitSingleThread,
};

use crate::{overlay::OverlayData, AppSession};

const HANDS: [Handed; 2] = [Handed::Left, Handed::Right];

pub const HAND_LEFT: usize = 0;
pub const HAND_RIGHT: usize = 1;

pub const POINTER_NORM: u16 = 0;
pub const POINTER_SHIFT: u16 = 1;
pub const POINTER_ALT: u16 = 2;

pub trait InteractionHandler {
    fn on_hover(&mut self, hit: &PointerHit);
    fn on_left(&mut self, hand: usize);
    fn on_press(&mut self, hit: &PointerHit);
    fn on_release(&mut self, hit: &PointerHit);
    fn on_scroll(&mut self, hit: &PointerHit, delta: f32);
}

pub struct InputState {
    pub hmd: Pose,
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
    grabbed_offset: (Vec3, Vec3),
    grabbed_idx: Option<usize>,
    hovered_idx: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerState {
    click: bool,
    grab: bool,
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
            hmd: Pose::IDENTITY,
            pointers: [PointerData::new(session, 0), PointerData::new(session, 1)],
        }
    }
    pub fn update(&mut self, sk: &SkDraw, interactables: &mut [OverlayData]) {
        self.hmd = sk.input_head();
        for h in 0..2 {
            self.pointers[h].update(&self.hmd, sk);
            self.pointers[h].test_interactions(&self.hmd, sk, interactables);
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
            grabbed_idx: None,
            grabbed_offset: (Vec3::ZERO, Vec3::ZERO),
            hovered_idx: None,
            colors: [session.color_norm, session.color_shift, session.color_alt],
        }
    }
    fn update(&mut self, hmd: &Pose, sk: &SkDraw) {
        let con = sk.input_controller(HANDS[self.hand]);
        self.pose = con.aim;

        self.before = self.now;
        self.now.click = con.trigger > 0.5;
        self.now.grab = con.grip > 0.5;
        self.now.show_hide = if self.hand == 0 {
            sk.input_controller_menu() == ButtonState::ACTIVE
        } else {
            false
        };
        self.now.scroll = con.stick.y;

        if self.before.click && !self.now.click {
            while let Some(action) = self.release_actions.pop_front() {
                action();
            }
        }

        let hmd_up = hmd.orientation.mul_vec3(Vec3::Y);
        let dot = hmd_up.dot(con.palm.forward());
        self.mode = if dot < -0.85 {
            POINTER_SHIFT // palm down
        } else if dot > 0.7 {
            POINTER_ALT // palm up
        } else {
            POINTER_NORM // neutral
        }
    }

    fn test_interactions(&mut self, hmd: &Pose, sk: &SkDraw, interactables: &mut [OverlayData]) {
        let color = self.colors[self.mode as usize];

        // Grabbing an overlay
        if let Some(grabbed_idx) = self.grabbed_idx {
            let grabbed = &mut interactables[grabbed_idx];
            if grabbed.primary_pointer != Some(self.hand) {
                debug!("Pointer {}: Grab lost on {}", self.hand, grabbed.name);
                self.grabbed_idx = None;
                // ignore and continue
            } else if !self.now.grab {
                debug!("Pointer {}: Dropped {}", self.hand, grabbed.name);
                self.grabbed_idx = None;
                grabbed.on_drop();
                // drop and continue
            } else {
                if self.now.scroll.abs() > 0.1 {
                    if self.mode == POINTER_ALT {
                        debug!("Pointer {}: Resize {}", self.hand, grabbed.name);
                        grabbed.on_size(self.now.scroll);
                    } else {
                        debug!("Pointer {}: Push/pull {}", self.hand, grabbed.name);
                        let offset = self.grabbed_offset.0
                            + self.grabbed_offset.0.normalize_or_zero() * self.now.scroll * 0.2;
                        let len_sq = offset.length_squared();
                        if len_sq > 0.20 && len_sq < 100. {
                            self.grabbed_offset.0 = offset;
                        }
                    }
                }
                let mat =
                    Affine3A::from_rotation_translation(self.pose.orientation, self.pose.position);
                sk.hierarchy_push(mat);
                let grab_point = sk.hierarchy_to_world_point(self.grabbed_offset.0);
                grabbed.on_move(grab_point, &hmd);
                sk.hierarchy_pop();
                sk.line_add(self.pose.position, grab_point, color, color, 0.002);

                if self.now.click && !self.before.click {
                    debug!("Pointer {}: on_curve {}", self.hand, grabbed.name);
                    grabbed.on_curve();
                }
                return;
            }
        }

        // Test for new hits
        let mut hits: [(usize, Vec2, f32); 8] = unsafe { MaybeUninit::zeroed().assume_init() };
        let mut num_hits = 0usize;

        for i in 0..interactables.len() {
            let overlay = &mut interactables[i];
            if let Some(gfx) = overlay.gfx.as_ref() {
                sk.hierarchy_push(overlay.transform);
                let ray = Ray::new(
                    sk.hierarchy_to_local_point(self.pose.position),
                    sk.hierarchy_to_local_direction(self.pose.forward()),
                );

                if let Some((hit, _)) = sk.mesh_ray_intersect(&gfx.mesh, ray, CullMode::Back) {
                    let vec = overlay.interaction_transform.transform_point3(hit.pos);
                    sk.line_add(ray.pos, hit.pos, color, color, 0.002);
                    hits[num_hits] = (i, vec2(vec.x, vec.y), Vec3::length(hit.pos - ray.pos));
                    num_hits += 1;
                    if num_hits > 7 {
                        sk.hierarchy_pop();
                        break;
                    }
                }
                sk.hierarchy_pop();
            }
        }

        if let Some(hit) = hits[..num_hits].iter().max_by(|a, b| a.2.total_cmp(&b.2)) {
            let now_idx = hit.0;
            let mut hit_data = PointerHit {
                hand: self.hand,
                mode: self.mode,
                uv: hit.1,
                dist: hit.2,
                primary: false,
            };

            // Invoke on_left
            if let Some(hovered_idx) = self.hovered_idx {
                if hovered_idx != now_idx {
                    let hovered = &mut interactables[hovered_idx];
                    if hovered.primary_pointer == Some(self.hand) {
                        hovered.primary_pointer = None;
                        debug!("Pointer {}: on_left {}", self.hand, hovered.name);
                        hovered.interaction.on_left(self.hand);
                    }
                }
            }
            self.hovered_idx = Some(now_idx);

            let overlay = &mut interactables[now_idx];

            // grab start
            if self.now.grab && !self.before.grab {
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

            overlay.interaction.on_hover(&hit_data);

            if self.now.click && !self.before.click {
                overlay.primary_pointer = Some(self.hand);
                hit_data.primary = true;
                overlay.interaction.on_press(&hit_data);
            } else if !self.now.click && self.before.click {
                overlay.interaction.on_release(&hit_data);
            }

            if self.now.scroll.abs() > 0.1 {
                overlay.interaction.on_scroll(&hit_data, self.now.scroll);
            }
        } else {
            // no hit
            if let Some(idx) = self.hovered_idx {
                let obj = &mut interactables[idx];
                if obj.primary_pointer == Some(self.hand) {
                    obj.primary_pointer = None;
                }
                obj.interaction.on_left(self.hand);
                self.hovered_idx = None;
            }
        }
    }
}

// --- Dummies & plumbing below ---

impl Default for PointerState {
    fn default() -> Self {
        Self {
            click: false,
            grab: false,
            show_hide: false,
            scroll: 0.,
        }
    }
}

pub struct DummyInteractionHandler;

impl InteractionHandler for DummyInteractionHandler {
    fn on_left(&mut self, _hand: usize) {}
    fn on_hover(&mut self, _hit: &crate::interactions::PointerHit) {}
    fn on_press(&mut self, _hit: &crate::interactions::PointerHit) {}
    fn on_scroll(&mut self, _hit: &crate::interactions::PointerHit, _delta: f32) {}
    fn on_release(&mut self, _hit: &crate::interactions::PointerHit) {}
}
