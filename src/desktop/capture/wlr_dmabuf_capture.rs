use std::sync::{Arc, Mutex};

use crate::{
    desktop::{
        frame::{texture_load_dmabuf, DmabufFrame, FRAME_FAILED, FRAME_READY},
        wl_client::{OutputState, WlClientState},
    },
    overlay::OverlayRenderer,
    AppState,
};
use log::{warn, debug};
use stereokit::StereoKitMultiThread;
use tokio::task::JoinHandle;

pub struct WlrDmabufCapture {
    output_idx: usize,
    wl: Arc<Mutex<WlClientState>>,
    task_handle: Option<JoinHandle<Arc<Mutex<DmabufFrame>>>>,
}

impl WlrDmabufCapture {
    pub fn try_new(wl: WlClientState, output: &OutputState) -> Option<Box<dyn OverlayRenderer>> {
        let mut output_idx = None;
        for i in 0..wl.outputs.len() {
            if wl.outputs[i].id == output.id {
                output_idx = Some(i);
                break;
            }
        }

        if let Some(output_idx) = output_idx {
            Some(Box::new(WlrDmabufCapture {
                output_idx,
                wl: Arc::new(Mutex::new(wl)),
                task_handle: None,
            }))
        } else {
            None
        }
    }
}

impl OverlayRenderer for WlrDmabufCapture {
    fn init(&mut self, _sk: &stereokit::SkDraw, _app: &mut AppState) {}
    fn pause(&mut self, app: &mut AppState) {
        if self.task_handle.is_some() {
            let handle = self.task_handle.take().unwrap();
            let _ = app.rt.block_on(handle);
        }
    }
    fn resume(&mut self, _app: &mut AppState) {}
    fn render(&mut self, sk: &stereokit::SkDraw, tex: &stereokit::Tex, app: &mut AppState) {
        if let Some(handle) = &self.task_handle {
            if handle.is_finished() {
                let handle = self.task_handle.take().unwrap();

                if let Ok(mutex) = app.rt.block_on(handle) {
                    if let Ok(frame) = mutex.lock() {
                        match frame.status {
                            FRAME_FAILED => {
                                warn!("[Dmabuf] Frame capture failed");
                            }
                            FRAME_READY => {
                                if frame.is_valid() {
                                    let handle =
                                        unsafe { sk.tex_get_surface(tex.as_ref()) as usize as u32 };
                                    texture_load_dmabuf(handle, &frame);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            } else {
                debug!("[Dmabuf] Frame not ready to present");
                return;
            }
        }

        let wl = self.wl.clone();
        let output_idx = self.output_idx;
        self.task_handle = Some(app.rt.spawn(async move {
            let frame = Arc::new(Mutex::new(DmabufFrame::default()));
            if let Ok(mut wl) = wl.lock() {
                wl.request_dmabuf_frame(output_idx, frame.clone());
            }
            frame
        }));
    }
}
