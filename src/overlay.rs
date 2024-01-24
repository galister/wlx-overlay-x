use std::sync::Arc;

use glam::{vec2, vec3, Affine3A, Mat3A, Quat, Vec3, Vec3A};
use log::info;
use stereokit::{
    sys::color32, Color128, Material, Mesh, RenderLayer, SkDraw, StereoKitDraw,
    StereoKitMultiThread, Tex, TextureFormat, TextureType, Vert,
};

use crate::{
    interactions::{DummyInteractionHandler, InteractionHandler},
    AppSession, AppState,
};

pub const COLOR_WHITE: Color128 = Color128 {
    r: 1.,
    g: 1.,
    b: 1.,
    a: 1.,
};
pub const COLOR_FALLBACK: Color128 = Color128 {
    r: 1.,
    g: 0.,
    b: 1.,
    a: 1.,
};
pub const COLOR_TRANSPARENT: Color128 = Color128 {
    r: 0.,
    g: 0.,
    b: 0.,
    a: 0.,
};

pub struct OverlayData {
    pub name: Arc<str>,
    pub width: f32,
    pub size: (i32, i32),
    pub visible: bool,
    pub want_visible: bool,
    pub show_hide: bool,
    pub grabbable: bool,
    pub color: Color128,
    pub transform: Affine3A,
    pub spawn_point: Vec3,
    pub spawn_rotation: Quat,
    pub relative_to: RelativeTo,
    pub interaction_transform: Affine3A,
    pub backend: Box<dyn OverlayBackend>,
    pub primary_pointer: Option<usize>,
    pub gfx: Option<OverlayGraphics>,
}

pub trait OverlayBackend: OverlayRenderer + InteractionHandler {}

pub struct OverlayGraphics {
    pub tex: Tex,
    pub mesh: Mesh,
    pub mat: Material,
}

pub trait OverlayRenderer {
    fn init(&mut self, sk: &SkDraw, app: &mut AppState);
    fn pause(&mut self, app: &mut AppState);
    fn resume(&mut self, app: &mut AppState);
    fn render(&mut self, sk: &SkDraw, tex: &Tex, app: &mut AppState);
}

impl OverlayData {
    pub fn show(&mut self, sk: &SkDraw, app: &mut AppState) {
        if self.visible {
            return;
        }

        info!("{}: Show", &self.name);

        self.visible = true;

        if self.gfx.is_none() {
            let tex = sk.tex_gen_color(
                COLOR_FALLBACK,
                self.size.0,
                self.size.1,
                TextureType::IMAGE_NO_MIPS,
                TextureFormat::RGBA32,
            );

            let mesh = sk.mesh_create();

            let scr_w = self.size.0 as f32;
            let scr_h = self.size.1 as f32;

            let half_w: f32;
            let half_h: f32;

            if scr_w >= scr_h {
                half_w = 1.;
                half_h = scr_h / scr_w;
            } else {
                half_w = scr_w / scr_h;
                half_h = 1.;
            }

            self.interaction_transform = Affine3A::from_scale_rotation_translation(
                vec3(0.5 / -half_w, 0.5 / -half_h, 0.),
                Quat::IDENTITY,
                vec3(0.5, 0.5, 0.),
            );

            let norm = vec3(0., 0., -1.);
            let col = color32::new_rgb(255, 255, 255);

            let x0 = 0f32;
            let x1 = 1f32;
            let y0 = 0f32;
            let y1 = 1f32;

            #[rustfmt::skip]
            let verts = vec![
                Vert { pos: vec3(-half_w, -half_h, 0.), uv: vec2(x1, y1), norm, col },
                Vert { pos: vec3(-half_w, half_h, 0.), uv: vec2(x1, y0), norm, col },
                Vert { pos: vec3(half_w, -half_h, 0.), uv: vec2(x0, y1), norm, col },
                Vert { pos: vec3(half_w, half_h, 0.), uv: vec2(x0, y0), norm, col },
            ];

            let inds = vec![0, 3, 2, 3, 0, 1];
            sk.mesh_set_verts(&mesh, &verts, true);
            sk.mesh_set_inds(&mesh, &inds);

            let mat = sk.material_create(&app.panel_shader);
            sk.material_set_texture(&mat, "diffuse", &tex);

            self.gfx = Some(OverlayGraphics { tex, mat, mesh });

            self.backend.init(sk, app);
        } else {
            self.backend.resume(app);
        }

        self.reset(app);
    }

    pub fn hide(&mut self, app: &mut AppState) {
        if !self.visible {
            return;
        }

        info!("{}: Hide", &self.name);

        self.visible = false;
        self.backend.pause(app);
    }

    pub fn reset(&mut self, app: &mut AppState) {
        let spawn = app.input.hmd.transform_point3(self.spawn_point);
        self.transform = Affine3A::from_translation(spawn);
        self.realign(&app.input.hmd)
    }

    pub fn render(&mut self, sk: &SkDraw, app: &mut AppState) {
        if !self.visible {
            return;
        }

        if let Some(gfx) = self.gfx.as_mut() {
            self.backend.render(sk, &gfx.tex, app);
            sk.mesh_draw(
                &gfx.mesh,
                &gfx.mat,
                self.transform,
                self.color,
                RenderLayer::LAYER0,
            );
        }
    }

    pub fn on_size(&mut self, delta: f32) {
        let t = self.transform.matrix3.mul_scalar(1. - delta.powi(3) * 0.05);
        let len_sq = t.x_axis.length_squared();
        if len_sq > 0.12 && len_sq < 100. {
            self.transform.matrix3 = t;
        }
    }

    pub fn on_move(&mut self, pos: Vec3A, hmd: &Affine3A) {
        if (hmd.translation - pos).length_squared() > 0.2 {
            self.transform.translation = pos;
            self.realign(hmd);
        }
    }

    pub fn on_drop(&mut self) {
        // TODO save position
    }

    pub fn on_curve(&mut self) {}

    pub fn realign(&mut self, hmd: &Affine3A) {
        let to_hmd = hmd.translation - self.transform.translation;
        let up_dir: Vec3A;

        if hmd.x_axis.dot(Vec3A::Y).abs() > 0.2 {
            // Snap upright
            up_dir = hmd.y_axis;
        } else {
            let dot = to_hmd.normalize().dot(hmd.z_axis);
            let z_dist = to_hmd.length();
            let y_dist = (self.transform.translation.y - hmd.translation.y).abs();
            let x_angle = (y_dist / z_dist).asin();

            if dot < -f32::EPSILON {
                // facing down
                let up_point = hmd.translation + z_dist / x_angle.cos() * Vec3A::Y;
                up_dir = (up_point - self.transform.translation).normalize();
            } else if dot > f32::EPSILON {
                // facing up
                let dn_point = hmd.translation + z_dist / x_angle.cos() * Vec3A::NEG_Y;
                up_dir = (self.transform.translation - dn_point).normalize();
            } else {
                // perfectly upright
                up_dir = Vec3A::Y;
            }
        }

        let scale = self.transform.x_axis.length();

        let col_z = (self.transform.translation - hmd.translation).normalize();
        let col_y = up_dir;
        let col_x = col_y.cross(col_z);
        let col_y = col_z.cross(col_x).normalize();
        let col_x = col_x.normalize();

        let rot = Mat3A::from_quat(self.spawn_rotation);
        self.transform.matrix3 = Mat3A::from_cols(col_x, col_y, col_z).mul_scalar(scale) * rot;
    }
}

// Boilerplate and dummies

pub struct SplitOverlayBackend {
    pub renderer: Box<dyn OverlayRenderer>,
    pub interaction: Box<dyn InteractionHandler>,
}

impl Default for SplitOverlayBackend {
    fn default() -> SplitOverlayBackend {
        SplitOverlayBackend {
            renderer: Box::new(FallbackRenderer),
            interaction: Box::new(DummyInteractionHandler),
        }
    }
}

impl OverlayBackend for SplitOverlayBackend {}
impl OverlayRenderer for SplitOverlayBackend {
    fn init(&mut self, sk: &SkDraw, app: &mut AppState) {
        self.renderer.init(sk, app);
    }
    fn pause(&mut self, app: &mut AppState) {
        self.renderer.pause(app);
    }
    fn resume(&mut self, app: &mut AppState) {
        self.renderer.resume(app);
    }
    fn render(&mut self, sk: &SkDraw, tex: &Tex, app: &mut AppState) {
        self.renderer.render(sk, tex, app);
    }
}
impl InteractionHandler for SplitOverlayBackend {
    fn on_left(&mut self, hand: usize) {
        self.interaction.on_left(hand);
    }
    fn on_hover(&mut self, hit: &crate::interactions::PointerHit) {
        self.interaction.on_hover(hit);
    }
    fn on_scroll(&mut self, hit: &crate::interactions::PointerHit, delta: f32) {
        self.interaction.on_scroll(hit, delta);
    }
    fn on_pointer(
        &mut self,
        session: &AppSession,
        hit: &crate::interactions::PointerHit,
        pressed: bool,
    ) {
        self.interaction.on_pointer(session, hit, pressed);
    }
}

impl Default for OverlayData {
    fn default() -> OverlayData {
        OverlayData {
            name: Arc::from(""),
            width: 1.,
            size: (0, 0),
            visible: false,
            want_visible: false,
            show_hide: false,
            grabbable: false,
            color: COLOR_WHITE,
            relative_to: RelativeTo::None,
            spawn_point: Vec3::NEG_Z,
            spawn_rotation: Quat::IDENTITY,
            transform: Affine3A::IDENTITY,
            interaction_transform: Affine3A::IDENTITY,
            gfx: None,
            backend: Box::<SplitOverlayBackend>::default(),
            primary_pointer: None,
        }
    }
}

pub struct FallbackRenderer;

impl OverlayRenderer for FallbackRenderer {
    fn init(&mut self, _sk: &SkDraw, _app: &mut AppState) {}
    fn pause(&mut self, _app: &mut AppState) {}
    fn resume(&mut self, _app: &mut AppState) {}
    fn render(&mut self, sk: &SkDraw, tex: &Tex, app: &mut AppState) {
        app.gl.begin_sk(sk, tex);
        app.gl.draw_color(
            vec3(1., 0., 1.),
            1.,
            0.,
            0.,
            sk.tex_get_width(tex) as _,
            sk.tex_get_height(tex) as _,
        );
        app.gl.end();
    }
}

pub enum RelativeTo {
    None,
    Head,
    Hand(usize),
}
