use glam::Affine3A;
use stereokit::{Color128, SkDraw};

use crate::AppState;

pub const COLOR_WHITE: Color128 = Color128 { r: 1., g: 1., b: 1., a: 1. };

pub trait Overlay {
    fn overlay(&self) -> &OverlayData;
    fn overlay_mut(&mut self) -> &mut OverlayData;

    fn show(&mut self, sk: &SkDraw);
    fn render(&mut self, sk: &SkDraw, state: &mut AppState);
}

pub struct OverlayData {
    pub visible: bool,
    pub want_visible: bool,
    pub color: Color128,
    pub transform: Affine3A,
}

impl OverlayData {
    pub fn new() -> OverlayData {
        OverlayData {
            visible: false,
            want_visible: false,
            color: COLOR_WHITE,
            transform: Affine3A::IDENTITY,
        }
    }
}

