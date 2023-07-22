use crate::desktop::{
    frame::{texture_load_dmabuf, FRAME_PENDING, FRAME_READY, FRAME_FAILED},
    wl_client::{OutputState, WlClientState},
};

use super::DesktopCapture;

pub struct WlrDmabufCapture {
    output_idx: usize,
    wl: WlClientState,
}

impl WlrDmabufCapture {
    pub fn new(wl: WlClientState, output: &OutputState) -> WlrDmabufCapture {
        let mut output_idx: usize = 420420;
        for i in 0..wl.outputs.len() {
            if wl.outputs[i].id == output.id {
                output_idx = i;
                break;
            }
        }
        debug_assert_ne!(output_idx, 420420);

        WlrDmabufCapture { wl, output_idx }
    }
}

impl DesktopCapture for WlrDmabufCapture {
    fn init(&mut self) {}
    fn pause(&mut self) {}
    fn resume(&mut self) {}
    fn render(&mut self, texture: u32) {
        if let Some(mutex) = self.wl.request_dmabuf_frame(self.output_idx) {
            if let Ok(frame) = mutex.lock() {
                match frame.status {
                    FRAME_PENDING => {
                        println!("[Dmabuf] Frame not ready to present");
                        return;
                    }
                    FRAME_FAILED => {
                        println!("[Dmabuf] Frame capture failed");
                    }
                    FRAME_READY => {
                        texture_load_dmabuf(texture, &frame);
                    }
                    _ => {}
                }
            }
        }
    }
}
