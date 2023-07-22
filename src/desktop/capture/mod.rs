pub mod pw_capture;
pub mod wlr_dmabuf_capture;
pub mod wlr_screencopy_capture;

pub trait DesktopCapture {
    fn init(&mut self);
    fn pause(&mut self);
    fn resume(&mut self);
    fn render(&mut self, texture: u32);
}
