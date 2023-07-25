use std::mem::transmute;

use chrono::Local;
use glam::{vec2, Affine3A, Quat, Vec3};
use stereokit::{Pose, StereoKitMultiThread, StereoKitSingleThread};

use crate::AppSession;

pub const WATCH_DEFAULT_POS: Vec3 = Vec3::new(0., -0.05, 0.05);
pub const WATCH_DEFAULT_ROT: Quat = Quat::from_xyzw(0., 1., 0., 0.);

pub struct WatchPanel {
    pub transform: Affine3A,
    pub hand: u32,
}

impl WatchPanel {
    pub fn new(session: &AppSession) -> WatchPanel {
        return WatchPanel {
            hand: session.watch_hand,
            transform: Affine3A::from_rotation_translation(
                session.watch_rot,
                session.watch_pos,
            ),
        };
    }

    pub fn render(&mut self, sk: &stereokit::SkDraw) {
        let cur_hand = sk.input_hand(unsafe { transmute(self.hand) });
        let mat =
            Affine3A::from_rotation_translation(cur_hand.palm.orientation, cur_hand.palm.position);
        sk.hierarchy_push(mat);
        sk.hierarchy_push(self.transform);

        sk.window(
            "Watch",
            Pose::IDENTITY,
            vec2(0.115, 0.0575),
            stereokit::WindowType::Body,
            stereokit::MoveType::Exact,
            |ui| {
                let date = Local::now();

                ui.label(format!("{}", &date.format("%H:%M")), true);
                ui.label(format!("{}", &date.format("%b %d")), true);
            },
        );

        sk.hierarchy_pop();
        sk.hierarchy_pop();
    }
}
