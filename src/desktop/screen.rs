use ashpd::zbus::export::futures_util::{FutureExt, TryFutureExt};
use glam::Mat4;
use stereokit::{
    Color128, Material, Mesh, Model, RenderLayer, SkDraw, StereoKitDraw, StereoKitMultiThread,
};

use crate::{
    gl::GlTexture,
    overlay::{Overlay, OverlayData},
    session::SESSION,
};

use super::wl_client::{OutputState, WlClientState};
use crate::desktop::capture::pw_capture::pipewire_initiate_capture;
use std::{fs, path::Path, task::Poll};

pub struct DesktopScreen {
    overlay: OverlayData,
    output: OutputState,
    wl: WlClientState,
    texture: GlTexture,
}

impl DesktopScreen {
    pub fn new(output: OutputState) -> DesktopScreen {
        DesktopScreen {
            overlay: OverlayData::new(),
            texture: GlTexture::new(output.size.0 as _, output.size.1 as _),
            output,
            wl: WlClientState::new(),
        }
    }

    pub async fn try_init(&mut self) -> bool {
        self.wl.maybe_wlr_dmabuf_mgr = None;
        if let Some(dmabuf_mgr) = &self.wl.maybe_wlr_dmabuf_mgr {
            println!("{}: Using Wlr DMA-Buf", &self.output.name);
            return true;
        } else {
            println!("{}: Using Pipewire capture", &self.output.name);
            let file_name = format!("{}.token", self.output.name);
            let full_path = Path::new(&SESSION.config_path).join(file_name);
            let token = fs::read_to_string(full_path).ok();

            if let Ok(node_id) = pipewire_initiate_capture(token.as_deref()).await {
                print!("Node id: {}", node_id);
                return true;
            }
        }

        false
    }

    pub fn render(&self, sk: &SkDraw) {
        if !self.overlay.visible {
            return;
        }

        sk.mesh_draw(
            Mesh::QUAD,
            Material::UNLIT,
            self.overlay.transform,
            self.overlay.color,
            RenderLayer::LAYER0,
        );
    }

    pub fn name(&self) -> &str {
        &self.output.name
    }
}

impl Overlay for DesktopScreen {
    fn overlay(&self) -> &OverlayData {
        &self.overlay
    }
}
