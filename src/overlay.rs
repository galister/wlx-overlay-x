use glam::Affine3A;
use stereokit::{Color128, SkDraw};

pub trait Overlay {
    fn overlay(&self) -> &OverlayData;
    fn overlay_mut(&mut self) -> &mut OverlayData;

    fn show(&mut self, sk: &SkDraw);
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
            color: Color128::new_rgb(1., 1., 1.),
            transform: Affine3A::IDENTITY,
        }
    }
}
