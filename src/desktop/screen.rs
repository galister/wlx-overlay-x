use glam::{Affine3A, Vec3, Vec3A};
use stereokit::{
    Color128, Material, Mesh, RenderLayer, SkDraw, StereoKitDraw, StereoKitMultiThread, Tex,
    TextureFormat, TextureType,
};

use crate::{
    desktop::capture::{
        wlr_dmabuf_capture::WlrDmabufCapture, wlr_screencopy_capture::WlrScreencopyCapture,
    },
    gl::GlTexture,
    overlay::{Overlay, OverlayData},
    session::SESSION,
    AppState,
};

use super::{
    capture::DesktopCapture,
    wl_client::{OutputState, WlClientState},
};
use crate::desktop::capture::pw_capture::pipewire_select_screen;
use std::{f32::consts::PI, fs, path::Path};

struct ScreenGraphics {
    tex: Tex,
    mat: Material,
}

pub struct DesktopScreen {
    overlay: OverlayData,
    output: OutputState,
    gfx: Option<ScreenGraphics>,
    capture: Option<Box<dyn DesktopCapture>>,
}

impl DesktopScreen {
    pub fn new(output: OutputState) -> DesktopScreen {
        DesktopScreen {
            overlay: OverlayData::new(),
            gfx: None,
            capture: None,
            output,
        }
    }

    pub async fn try_init(&mut self, dmabuf: bool) -> bool {
        println!(
            "{}: Res {}x{} Size {}x{} Pos {}x{}",
            &self.output.name,
            &self.output.size.0,
            &self.output.size.1,
            &self.output.logical_size.0,
            &self.output.logical_size.1,
            &self.output.logical_pos.0,
            &self.output.logical_pos.1
        );

        if true {
            println!("{}: Using Wlr Screencopy", &self.output.name);
            let wl = WlClientState::new();

            let capture = WlrScreencopyCapture::new(wl, &self.output);
            self.capture = Some(Box::new(capture));

            return true;
        } else if dmabuf {
            println!("{}: Using Wlr DMA-Buf", &self.output.name);
            let wl = WlClientState::new();

            let capture = WlrDmabufCapture::new(wl, &self.output);
            self.capture = Some(Box::new(capture));

            return true;
        } else {
            println!("{}: Using Pipewire capture", &self.output.name);
            let file_name = format!("{}.token", self.output.name);
            let full_path = Path::new(&SESSION.config_path).join(file_name);
            let token = fs::read_to_string(full_path).ok();

            if let Ok(node_id) = pipewire_select_screen(token.as_deref()).await {
                print!("Node id: {}", node_id);
                return true;
            }
        }

        false
    }

    pub fn render(&mut self, sk: &SkDraw, state: &mut AppState) {
        if !self.overlay.visible {
            return;
        }

        let gfx = self.gfx.as_mut().unwrap();

        if let Some(capture) = self.capture.as_mut() {
            let w = self.output.size.0 as f32;
            let h = self.output.size.1 as f32;
            let wi = self.output.size.0;
            let hi = self.output.size.1;

            let sk_tex = sk.tex_gen_color(
                Color128::new_rgb(0., 0., 1.),
                wi,
                hi,
                TextureType::IMAGE_NO_MIPS,
                TextureFormat::RGBA32,
            );
            let _handle = unsafe { sk.tex_get_surface(&sk_tex) as usize as u32 };

            let mut gl_tex = GlTexture::new();
            gl_tex.width = wi as _;
            gl_tex.height = hi as _;
            capture.render(gl_tex.handle);

            //let data: Vec<u8> = vec![255, 0, 0, 255];
            //gl_tex.allocate(1, 1, GL_SRGB8_ALPHA8 as _, data.as_ptr());

            //let handle = unsafe { sk.tex_get_surface(&gfx.tex) as usize as u32 };
            //capture.render(handle);

            state.renderer.begin_sk(sk, &mut gfx.tex);
            let col0 = Vec3::new(0., 1., 1.);
            let col1 = Vec3::new(1., 0., 1.);
            let col2 = Vec3::new(1., 1., 0.);
            state.renderer.draw_color(col0, 0., h / 2., w / 2., h / 2.);
            state
                .renderer
                .draw_color(col2, w / 2., h / 2., w / 2., h / 2.);
            state.renderer.draw_color(col1, w / 2., 0., w / 2., h / 2.);
            state
                .renderer
                .draw_sprite(&gl_tex, w / 4., h / 4., w / 2., h / 2.);
            state.renderer.end();
        }

        sk.mesh_draw(
            Mesh::QUAD,
            &gfx.mat,
            self.overlay.transform,
            self.overlay.color,
            RenderLayer::LAYER0,
        );
    }
}

impl Overlay for DesktopScreen {
    fn overlay(&self) -> &OverlayData {
        &self.overlay
    }
    fn overlay_mut(&mut self) -> &mut OverlayData {
        &mut self.overlay
    }
    fn show(&mut self, sk: &SkDraw) {
        if self.overlay.visible {
            return;
        }

        println!("{}: Show", &self.output.name);

        self.overlay.visible = true;

        if self.gfx.is_none() {
            let tex = sk.tex_gen_color(
                Color128::new_rgb(1., 0., 1.),
                self.output.size.0,
                self.output.size.1,
                TextureType::IMAGE_NO_MIPS,
                TextureFormat::RGBA32,
            );

            let mat = sk.material_copy(Material::UNLIT);
            sk.material_set_texture(&mat, "diffuse", &tex);

            self.gfx = Some(ScreenGraphics { tex, mat });
        }

        println!(
            "Head at {}, looking {}",
            sk.input_head().position,
            sk.input_head().forward()
        );

        let forward = sk.input_head().position + sk.input_head().forward();
        self.overlay.transform.translation = forward.into();

        self.overlay.transform = Affine3A::from_rotation_y(PI);
        self.overlay.transform.translation = forward.into();

        println!(
            "Overlay at {}, looking at {}",
            forward,
            self.overlay.transform.transform_vector3a(-Vec3A::Z)
        );
    }
}
