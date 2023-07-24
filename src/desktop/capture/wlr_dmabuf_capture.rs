use crate::{
    desktop::{
        frame::{texture_load_dmabuf, FRAME_FAILED, FRAME_PENDING, FRAME_READY},
        wl_client::{OutputState, WlClientState},
    },
    overlay::{COLOR_FALLBACK, OverlayRenderer}, AppState,
};
use log::warn;
use stereokit::{StereoKitMultiThread, Tex, TextureFormat, TextureType};

pub struct WlrDmabufCapture {
    output_idx: usize,
    wl: WlClientState,
    staging_tex: Option<Tex>,
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
                wl,
                output_idx,
                staging_tex: None,
            }))
        } else {
            None
        }
    }
}

impl OverlayRenderer for WlrDmabufCapture {
    fn init(&mut self, sk: &stereokit::SkDraw) {
        let size = self.wl.outputs[self.output_idx].size;
        self.staging_tex = Some(sk.tex_gen_color(
            COLOR_FALLBACK,
            size.0,
            size.1,
            TextureType::IMAGE_NO_MIPS,
            TextureFormat::RGBA32,
        ));
    }
    fn pause(&mut self) {}
    fn resume(&mut self) {}
    fn render(&mut self, sk: &stereokit::SkDraw, tex: &stereokit::Tex, app: &mut AppState) {
        if let Some(mutex) = self.wl.request_dmabuf_frame(self.output_idx) {
            if let Ok(frame) = mutex.lock() {
                match frame.status {
                    FRAME_PENDING => {
                        warn!("[Dmabuf] Frame not ready to present");
                        return;
                    }
                    FRAME_FAILED => {
                        warn!("[Dmabuf] Frame capture failed");
                    }
                    FRAME_READY => {
                        let handle =
                            unsafe { sk.tex_get_surface(self.staging_tex.as_ref().unwrap()) as usize as u32 };
                        texture_load_dmabuf(handle, &frame);
                        app.gl.begin_sk(sk, &tex);
                        app.gl.srgb_correction(handle);
                        app.gl.end();
                    }
                    _ => {}
                }
            }
        }
    }
}
