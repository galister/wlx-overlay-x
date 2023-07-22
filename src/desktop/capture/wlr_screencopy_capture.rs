use std::sync::{Mutex, Arc};

use crate::desktop::{
    frame::{texture_load_memfd, FRAME_PENDING, FRAME_READY, FRAME_FAILED, MemFdFrame},
    wl_client::{OutputState, WlClientState},
};

use super::DesktopCapture;

pub struct WlrScreencopyCapture {
    output_idx: usize,
    wl: WlClientState,
    frame: Option<Arc<Mutex<MemFdFrame>>>
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

        WlrScreencopyCapture { wl, output_idx, frame: None }
    }
}

impl DesktopCapture for WlrScreencopyCapture {
    fn init(&mut self) {}
    fn pause(&mut self) {}
    fn resume(&mut self) {}
    fn render(&mut self, texture: u32) {
        if let Some(mutex) = self.frame.as_ref() { 
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
                        texture_load_memfd(texture, &frame);
                    }
                    _ => {}
                }
            }
        }
        self.frame = self.wl.request_screencopy_frame(self.output_idx);
    }
}
