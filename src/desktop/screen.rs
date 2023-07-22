use glam::{Affine3A, Vec3A, vec3, vec2};
use stereokit::{
    Color128, Material, Mesh, RenderLayer, SkDraw, StereoKitDraw, StereoKitMultiThread, Tex,
    TextureFormat, TextureType, Vert, sys::color32,
};

use crate::{
    desktop::capture::{
        wlr_dmabuf_capture::WlrDmabufCapture, wlr_screencopy_capture::WlrScreencopyCapture,
    },
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
    mesh: Mesh,
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

            if let Ok(session) = SESSION.lock() {
                println!("{}: Using Pipewire capture", &self.output.name);
                let file_name = format!("{}.token", self.output.name);
                let full_path = Path::new(&session.config_path).join(file_name);
                let token = fs::read_to_string(full_path).ok();

                if let Ok(node_id) = pipewire_select_screen(token.as_deref()).await {
                    print!("Node id: {}", node_id);
                    return true;
                }
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
            let handle = unsafe { sk.tex_get_surface(&gfx.tex) as usize as u32 };
            capture.render(handle);

            //let data: Vec<u8> = vec![255, 0, 0, 255];
            //gl_tex.allocate(1, 1, GL_SRGB8_ALPHA8 as _, data.as_ptr());

            //let handle = unsafe { sk.tex_get_surface(&gfx.tex) as usize as u32 };
            //capture.render(handle);

            /*
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
            */
        }

        sk.mesh_draw(
            &gfx.mesh,
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

            let mesh = sk.mesh_create();

            let scr_w = self.output.size.0 as f32;
            let scr_h = self.output.size.1 as f32;

            let half_w: f32;
            let half_h: f32;

            if scr_w >= scr_h {
                half_w = 1.;
                half_h = scr_h / scr_w;
            } else {
                half_w = scr_w / scr_h;
                half_h = 1.;
            }

            let norm = vec3(0., 0., -1.);
            let col = color32::new_rgb(255, 255, 255);

            let mut x0 = 0f32;
            let mut x1 = 1f32;
            let mut y0 = 0f32;
            let mut y1 = 1f32;

            if let Ok(session) = SESSION.lock() {
                if session.screen_flip_h {
                    x0 = 1.;
                    x1 = 0.;
                }
                if session.screen_flip_v {
                    y0 = 1.;
                    y1 = 0.;
                }
            }

            #[rustfmt::skip]
            let verts = vec![
                Vert { pos: vec3(-half_w, -half_h, 0.), uv: vec2(x0, y1), norm, col },
                Vert { pos: vec3(-half_w, half_h, 0.), uv: vec2(x0, y0), norm, col },
                Vert { pos: vec3(half_w, -half_h, 0.), uv: vec2(x1, y1), norm, col },
                Vert { pos: vec3(half_w, half_h, 0.), uv: vec2(x1, y0), norm, col },
            ];
            let inds = vec![1, 2, 0, 2, 1, 3];
            sk.mesh_set_verts(&mesh, &verts, true);
            sk.mesh_set_inds(&mesh, &inds);

            let mat = sk.material_copy(Material::UNLIT);
            sk.material_set_texture(&mat, "diffuse", &tex);

            self.gfx = Some(ScreenGraphics { tex, mat, mesh });
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
