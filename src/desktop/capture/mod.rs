pub mod pw_capture;
pub mod wlr_capture;

use crate::gl::GlTexture;

trait DesktopCapture {
    fn init();
    fn pause();
    fn resume();
    fn render(texture: &GlTexture);
}
