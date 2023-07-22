use crate::{
    desktop::wl_client::{OutputState, WlClientState},
    gl::{
        dmabuf::{FRAME_FAILED, FRAME_PENDING, FRAME_READY},
        memfd::texture_load_memfd,
    },
};

use super::DesktopCapture;

pub struct WlrScreencopyCapture {
    output_idx: usize,
    wl: WlClientState,
}

impl WlrScreencopyCapture {
    pub fn new(wl: WlClientState, output: &OutputState) -> WlrScreencopyCapture {
        let mut output_idx: usize = 420420;
        for i in 0..wl.outputs.len() {
            if wl.outputs[i].id == output.id {
                output_idx = i;
                break;
            }
        }
        debug_assert_ne!(output_idx, 420420);

        WlrScreencopyCapture { wl, output_idx }
    }
}

impl DesktopCapture for WlrScreencopyCapture {
    fn init(&mut self) {}
    fn pause(&mut self) {}
    fn resume(&mut self) {}
    fn render(&mut self, texture: u32) {
        match self.wl.memfd_frame.status {
            FRAME_PENDING => {
                println!("[Dmabuf] Frame not ready to present");
                return;
            }
            FRAME_FAILED => {
                println!("[Dmabuf] Frame capture failed");
            }
            FRAME_READY => {
                texture_load_memfd(texture, &self.wl.memfd_frame);
            }
            _ => {}
        }

        self.wl.memfd_frame.status = FRAME_PENDING;
        self.wl.request_screencopy_frame(self.output_idx);
    }
}
