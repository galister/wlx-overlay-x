use glam::{Vec2, Vec3};
use stereokit::{SkDraw, StereoKitMultiThread, Tex, TextureFormat, TextureType};

use crate::{overlay::OverlayRenderer, AppState};

pub mod font;

const RES_DIVIDER: usize = 4;

struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

struct CanvasGl {
    tex_bg: Tex,
    tex_hl: Tex,
    tex_fg: Tex,
}

// Parses a color from a HTML hex string
pub fn color_parse(html_hex: &str) -> Vec3 {
    let mut color = Vec3::ZERO;
    color.x = u8::from_str_radix(&html_hex[1..3], 16).unwrap() as f32 / 255.;
    color.y = u8::from_str_radix(&html_hex[3..5], 16).unwrap() as f32 / 255.;
    color.z = u8::from_str_radix(&html_hex[5..7], 16).unwrap() as f32 / 255.;
    color
}

pub struct Canvas<T> {
    pub width: usize,
    pub height: usize,
    pub controls: Vec<Control<T>>,

    pub fg_color: Vec3,
    pub bg_color: Vec3,
    pub font_size: isize,

    interact_map: Vec<u8>,
    interact_stride: usize,
    interact_rows: usize,

    hover_controls: [Option<usize>; 2],

    gl: Option<CanvasGl>,
}

impl<T> Canvas<T> {
    pub fn new(width: usize, height: usize) -> Self {
        let stride = width / RES_DIVIDER;
        let rows = height / RES_DIVIDER;

        Self {
            width,
            height,
            interact_map: vec![0; stride * rows],
            interact_stride: stride,
            interact_rows: rows,
            controls: Vec::new(),
            bg_color: Vec3::ZERO,
            fg_color: Vec3::ONE,
            font_size: 16,
            hover_controls: [None, None],
            gl: None,
        }
    }

    // Creates a panel with bg_color inherited from the canvas
    pub fn panel(&mut self, x: f32, y: f32, w: f32, h: f32) -> usize {
        self.controls.push(Control {
            rect: Rect { x, y, w, h },
            bg_color: self.bg_color,
            on_render_bg: Some(Control::render_rect),
            ..Default::default()
        });
        self.controls.len() - 1
    }

    // Creates a label with fg_color, font_size inherited from the canvas
    pub fn label(&mut self, x: f32, y: f32, w: f32, h: f32, text: String) -> usize {
        self.controls.push(Control {
            rect: Rect { x, y, w, h },
            text,
            fg_color: self.fg_color,
            size: self.font_size,
            on_render_fg: Some(Control::render_text),
            ..Default::default()
        });
        self.controls.len() - 1
    }

    // Creates a label with fg_color, font_size inherited from the canvas
    pub fn label_centered(&mut self, x: f32, y: f32, w: f32, h: f32, text: String) -> usize {
        self.controls.push(Control {
            rect: Rect { x, y, w, h },
            text,
            fg_color: self.fg_color,
            size: self.font_size,
            on_render_fg: Some(Control::render_text_centered),
            ..Default::default()
        });
        self.controls.len() - 1
    }

    // Creates a button with fg_color, bg_color, font_size inherited from the canvas
    pub fn button(&mut self, x: f32, y: f32, w: f32, h: f32, text: String) -> usize {
        let idx = self.controls.len();

        self.interactive_set_idx(x, y, w, h, idx);

        self.controls.push(Control {
            rect: Rect { x, y, w, h },
            text,
            fg_color: self.fg_color,
            bg_color: self.bg_color,
            size: self.font_size,
            on_render_bg: Some(Control::render_rect),
            on_render_fg: Some(Control::render_text_centered),
            ..Default::default()
        });
        self.controls.len() - 1
    }

    fn interactive_set_idx(&mut self, x: f32, y: f32, w: f32, h: f32, idx: usize) {
        let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);

        let x_min = (x / RES_DIVIDER).max(0);
        let y_min = (y / RES_DIVIDER).max(0);
        let x_max = (x_min + (w / RES_DIVIDER)).min(self.interact_stride - 1);
        let y_max = (y_min + (h / RES_DIVIDER)).min(self.interact_rows - 1);

        for y in y_min..y_max {
            for x in x_min..x_max {
                self.interact_map[y * self.interact_stride + x] = idx as u8;
            }
        }
    }

    fn interactive_get_idx(&self, uv: Vec2) -> usize {
        let x = (uv.x as usize) * self.width;
        let y = (uv.y as usize) * self.height;
        let x = (x / RES_DIVIDER).max(0).min(self.interact_stride - 1);
        let y = (y / RES_DIVIDER).max(0).min(self.interact_rows - 1);
        self.interact_map[y * self.interact_stride + x] as usize
    }

    pub fn on_left(&mut self, hand: usize) {
        self.hover_controls[hand] = None;
    }
    pub fn on_hover(&mut self, hit: &crate::interactions::PointerHit) {
        let i = self.interactive_get_idx(hit.uv);
        if i > 0 {
            self.hover_controls[hit.hand] = Some(i);
        }
    }
    pub fn on_pointer(&mut self, hit: &crate::interactions::PointerHit, pressed: bool) -> Option<T> {
        let i = self.interactive_get_idx(hit.uv);
        if i > 0 {
            self.hover_controls[hit.hand] = Some(i);
            let c = &mut self.controls[i];
            if pressed {
                if let Some(ref mut f) = c.on_press {
                    return f(c);
                }
            } else if let Some(ref mut f) = c.on_release {
                return f(c);
            }
        }
        None
    }
    pub fn on_scroll(&mut self, _hit: &crate::interactions::PointerHit, _delta: f32) -> Option<T> {
        None
    }
}

impl<T> OverlayRenderer for Canvas<T> {
    fn init(&mut self, sk: &stereokit::SkDraw, app: &mut AppState) {
        let gl = CanvasGl {
            tex_bg: sk.tex_create(TextureType::IMAGE_NO_MIPS, TextureFormat::RGBA32),
            tex_hl: sk.tex_create(TextureType::IMAGE_NO_MIPS, TextureFormat::RGBA32),
            tex_fg: sk.tex_create(TextureType::IMAGE_NO_MIPS, TextureFormat::RGBA32),
        };

        app.gl.begin_sk(sk, &gl.tex_bg);
        app.gl.clear();
        for c in self.controls.iter_mut() {
            if let Some(fun) = c.on_render_bg {
                fun(c, sk, app);
            }
        }
        app.gl.end();

        app.gl.begin_sk(sk, &gl.tex_fg);
        app.gl.clear();
        for c in self.controls.iter_mut() {
            if let Some(fun) = c.on_render_fg {
                fun(c, sk, app);
            }
        }
        app.gl.end();

        self.gl = Some(gl);
    }
    fn pause(&mut self, _app: &mut AppState) {}
    fn resume(&mut self, _app: &mut AppState) {}
    fn render(&mut self, sk: &stereokit::SkDraw, tex: &Tex, app: &mut AppState) {
        if self.gl.is_none() {
            return;
        }
        let gl = self.gl.as_ref().unwrap();

        app.gl.begin_sk(sk, tex);

        // static background
        let handle = unsafe { sk.tex_get_surface(gl.tex_bg.as_ref()) } as usize as u32;
        app.gl.draw_sprite_full(handle);

        // interactive highlights etc
        let handle = unsafe { sk.tex_get_surface(gl.tex_hl.as_ref()) } as usize as u32;
        app.gl.draw_sprite_full(handle);

        // mostly static text
        let handle = unsafe { sk.tex_get_surface(gl.tex_fg.as_ref()) } as usize as u32;
        app.gl.draw_sprite_full(handle);

        app.gl.end();
    }
}

pub struct Control<T> {
    pub data: Vec<usize>,
    rect: Rect,
    fg_color: Vec3,
    bg_color: Vec3,
    text: String,
    size: isize,
    dirty: bool,

    pub on_update: Option<fn(&mut Self)>,
    pub on_press: Option<fn(&mut Self) -> Option<T>>,
    pub on_release: Option<fn(&mut Self) -> Option<T>>,

    on_render_bg: Option<fn(&mut Self, &SkDraw, &mut AppState)>,
    on_render_hl: Option<fn(&mut Self, &SkDraw, &mut AppState, bool)>,
    on_render_fg: Option<fn(&mut Self, &SkDraw, &mut AppState)>,
}

impl<T> Default for Control<T> {
    fn default() -> Self {
        Self {
            rect: Rect {
                x: 0.,
                y: 0.,
                w: 0.,
                h: 0.,
            },
            fg_color: Vec3::ONE,
            bg_color: Vec3::ZERO,
            text: String::new(),
            dirty: false,
            size: 24,
            data: vec![],
            on_update: None,
            on_render_bg: None,
            on_render_hl: None,
            on_render_fg: None,
            on_press: None,
            on_release: None,
        }
    }
}

impl<T> Control<T> {
    #[inline(always)]
    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        self.text = text;
        self.dirty = true;
    }

    #[inline(always)]
    pub fn get_text(&self) -> &str {
        &self.text
    }

    fn render_rect(&mut self, _sk: &SkDraw, app: &mut AppState) {
        app.gl.draw_color(
            self.bg_color,
            1.,
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
        );
    }

    fn render_highlight(&mut self, _sk: &SkDraw, app: &mut AppState) {
        app.gl.draw_color(
            Vec3::ONE,
            0.3,
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
        );
    }

    fn render_text(&mut self, sk: &SkDraw, app: &mut AppState) {
        let mut cur_y = self.rect.y;
        for line in self.text.lines() {
            let mut cur_x = self.rect.x;
            for glyph in app.fc.get_glyphs(line, self.size, sk) {
                if let Some(tex) = &glyph.tex {
                    let handle = unsafe { sk.tex_get_surface(tex.as_ref()) } as usize as u32;
                    app.gl.draw_glyph(
                        handle,
                        cur_x + glyph.left,
                        cur_y + glyph.top,
                        glyph.width,
                        glyph.height,
                        self.fg_color,
                    );
                }

                cur_x += glyph.advance;
            }
            cur_y += (self.size as f32) * 1.5;
        }
    }
    fn render_text_centered(&mut self, sk: &SkDraw, app: &mut AppState) {
        let (w, h) = app.fc.get_text_size(&self.text, self.size, sk);

        let mut cur_y = self.rect.y + (self.rect.h * 0.5) - (h * 0.5);
        for line in self.text.lines() {
            let mut cur_x = self.rect.x + (self.rect.w * 0.5) - (w * 0.5);
            for glyph in app.fc.get_glyphs(line, self.size, sk) {
                if let Some(tex) = &glyph.tex {
                    let handle = unsafe { sk.tex_get_surface(tex.as_ref()) } as usize as u32;
                    app.gl.draw_glyph(
                        handle,
                        cur_x + glyph.left,
                        cur_y + glyph.top,
                        glyph.width,
                        glyph.height,
                        self.fg_color,
                    );
                }

                cur_x += glyph.advance;
            }
            cur_y += (self.size as f32) * 1.5;
        }
    }
}
