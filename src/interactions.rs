use std::{collections::VecDeque, mem::MaybeUninit};

use glam::{vec2, Affine3A, Vec2, Vec3};
use stereokit::{
    ButtonState, CullMode, Handed, InputSource, Pose, Ray, SkDraw, StereoKitMultiThread,
    StereoKitSingleThread,
};

use crate::overlay::OverlayData;

const HANDS: [Handed; 2] = [Handed::Left, Handed::Right];
const HAND_SOURCES: [InputSource; 2] = [InputSource::HAND_LEFT, InputSource::HAND_RIGHT];

pub const HAND_LEFT: usize = 0;
pub const HAND_RIGHT: usize = 0;

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
    hmd: Pose,
    pointers: [PointerData; 2],
}

pub struct PointerData {
    hand: usize,
    release_actions: VecDeque<Box<dyn Fn()>>,
    now: PointerState,
    before: PointerState,
    mode: u16,
    pose: Pose,
    grabbed_offset: Vec3,
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
    pub fn new() -> Self {
        Self {
            hmd: Pose::IDENTITY,
            pointers: [PointerData::new(HAND_LEFT), PointerData::new(HAND_RIGHT)],
        }
    }
    pub fn update(&mut self, sk: &SkDraw, interactables: &mut [OverlayData]) {
        self.hmd = sk.input_head();
        for h in 0..2 {
            self.pointers[h].update(&self.hmd, sk);
            self.pointers[h].test_interactions(sk, interactables);
        }
    }
}

impl PointerData {
    fn new(hand: usize) -> Self {
        PointerData {
            hand,
            release_actions: VecDeque::new(),
            now: PointerState::default(),
            before: PointerState::default(),
            mode: 0,
            pose: Pose::IDENTITY,
            grabbed_idx: None,
            grabbed_offset: Vec3::ZERO,
            hovered_idx: None,
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

    fn test_interactions(&mut self, sk: &SkDraw, interactables: &mut [OverlayData]) {
        // Grabbing an overlay
        if let Some(grabbed_idx) = self.grabbed_idx {
            let grabbed = &mut interactables[grabbed_idx];
            if grabbed.primary_pointer != Some(self.hand) {
                self.grabbed_idx = None;
                // ignore and continue
            } else if !self.now.grab {
                self.grabbed_idx = None;
                grabbed.on_drop();
                // drop and continue
            } else {
                if self.now.scroll.abs() > 0.1 {
                    if self.mode == POINTER_ALT {
                        grabbed.on_size(self.now.scroll);
                    } else {
                        let offset = self.grabbed_offset
                            + self.grabbed_offset.normalize_or_zero() * self.now.scroll.powi(2);
                        let len_sq = offset.length_squared();
                        if len_sq > 0.09 && len_sq < 100. {
                            self.grabbed_offset = offset;
                        }
                    }
                }
                grabbed.on_move(self.pose.position + self.pose.forward() * self.grabbed_offset);
                if self.now.click && !self.before.click {
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

        if let Some(hit) = hits[..num_hits].iter().max_by(|a,b| {a.2.total_cmp(&b.2)}) {
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
                    let obj = &mut interactables[hovered_idx];
                    if obj.primary_pointer == Some(self.hand) {
                        obj.primary_pointer = None;
                        obj.interaction.on_left(self.hand);
                    }
                }
            }
            self.hovered_idx = Some(now_idx);

            let overlay = &mut interactables[now_idx];

            // grab start
            if self.now.grab && !self.before.grab {
                overlay.primary_pointer = Some(self.hand);
                let mat = Affine3A::from_rotation_translation(self.pose.orientation, self.pose.position).inverse();
                self.grabbed_offset = mat.transform_point3(overlay.transform.translation.into());
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
    fn on_left(&mut self, _hand: usize) { }
    fn on_hover(&mut self, _hit: &crate::interactions::PointerHit) { }
    fn on_press(&mut self, _hit: &crate::interactions::PointerHit) { }
    fn on_scroll(&mut self, _hit: &crate::interactions::PointerHit, _delta: f32) { }
    fn on_release(&mut self, _hit: &crate::interactions::PointerHit) { }
}
