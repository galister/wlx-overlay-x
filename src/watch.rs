use std::mem::transmute;

use chrono::Local;
use glam::{Affine3A, Vec3, Quat, vec2};
use stereokit::{StereoKitMultiThread, StereoKitSingleThread, Pose};

use crate::{overlay::{OverlayData, Overlay, COLOR_WHITE}, AppState, session::SESSION};

pub const WATCH_DEFAULT_POS: Vec3 = Vec3::new(0., -0.05, 0.05);
pub const WATCH_DEFAULT_ROT: Quat = Quat::from_xyzw(0., 1., 0., 0.);

pub struct WatchPanel {
    pub overlay: OverlayData,
    pub hand: u32,
}

impl WatchPanel {
    pub fn new() -> WatchPanel {
        if let Ok(session) = SESSION.lock() {
            return WatchPanel {
                hand: session.watch_hand,
                overlay: OverlayData { 
                    visible: false , 
                    want_visible: true, 
                    color: COLOR_WHITE, 
                    transform: Affine3A::from_rotation_translation(session.watch_rot, session.watch_pos),
                }
            }
        }
        panic!("Could not get session.");
    }
}

impl Overlay for WatchPanel {
    fn show(&mut self, sk: &stereokit::SkDraw) {
        
    }
    fn render(&mut self, sk: &stereokit::SkDraw, _state: &mut AppState) {
        let cur_hand = sk.input_hand(unsafe { transmute(self.hand) });
        let mat = Affine3A::from_rotation_translation(cur_hand.palm.orientation, cur_hand.palm.position);
        sk.hierarchy_push(mat);
        sk.hierarchy_push(self.overlay.transform);

        sk.window("Watch", Pose::IDENTITY, vec2(0.115, 0.0575), stereokit::WindowType::Body, stereokit::MoveType::Exact, |ui| {
            let date = Local::now();

            ui.label(format!("{}", &date.format("%H:%M")), true);
            ui.label(format!("{}", &date.format("%b %d")), true);
        }); 

        sk.hierarchy_pop();
        sk.hierarchy_pop();
    }
    fn overlay(&self) -> &OverlayData {
        &self.overlay
    }
    fn overlay_mut(&mut self) -> &mut OverlayData {
        &mut self.overlay
    }
}
