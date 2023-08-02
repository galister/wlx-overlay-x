use glam::Vec3;
use stereokit::{Tex, StereoKitMultiThread, TextureType, SkDraw};

use crate::{AppState, overlay::OverlayRenderer};

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

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    controls: Vec<Control>,

    interact_map: Vec<u8>,
    interact_stride: usize,
    interact_rows: usize,
    
    gl: Option<CanvasGl>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Canvas {
        let stride = width / RES_DIVIDER;
        let rows = height / RES_DIVIDER;

        Canvas {
            width,
            height,
            interact_map: vec![0; stride * rows],
            interact_stride: stride,
            interact_rows: rows,
            controls: Vec::new(),
            gl: None,
        }
    }
    pub fn panel(&mut self, x: f32, y: f32, w: f32, h: f32, color: Vec3) {
        self.controls.push(
            Control { 
                rect: Rect { x, y, w, h }, 
                color,
                on_render_bg: Some(Control::render_rect),
                ..Default::default()
            });
    }
    pub fn label(&mut self, x: f32, y: f32, w: f32, h: f32, text: String) {
        self.controls.push(
            Control { 
                rect: Rect { x, y, w, h }, 
                text,
                on_render_fg: Some(Control::render_text),
                ..Default::default()
            });
    }
    pub fn label_centered(&mut self, x: f32, y: f32, w: f32, h: f32, text: String) {
        self.controls.push(
            Control { 
                rect: Rect { x, y, w, h }, 
                text,
                on_render_fg: Some(Control::render_text_centered),
                ..Default::default()
            });
    }

    pub fn button(&mut self, x: f32, y: f32, w: f32, h: f32, text: String) {
        let idx = self.controls.len();
        
        self.interactive_set_idx(x, y, w, h, idx);

        self.controls.push(
            Control { 
                rect: Rect { x, y, w, h }, 
                text,
                on_render_bg: Some(Control::render_rect),
                on_render_fg: Some(Control::render_text_centered),
                ..Default::default()
            });
    }

    fn interactive_set_idx(&mut self, x: f32, y: f32, w: f32, h: f32, idx: usize) {
        let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);

        let x_min = (x/RES_DIVIDER).max(0);
        let y_min = (y/RES_DIVIDER).max(0);
        let x_max = (x_min + (w/RES_DIVIDER)).min(self.interact_stride-1);
        let y_max = (y_min + (h/RES_DIVIDER)).min(self.interact_rows-1);

        for y in y_min..y_max {
            for x in x_min..x_max {
                self.interact_map[y * self.interact_stride + x] = idx as u8;
            }
        }
    }

    fn interactive_get_idx(&self, x: f32, y: f32) -> usize {
        let (x, y) = (x as usize, y as usize);
        let x = (x/RES_DIVIDER).max(0).min(self.interact_stride-1);
        let y = (y/RES_DIVIDER).max(0).min(self.interact_rows-1);
        self.interact_map[y * self.interact_stride + x] as usize
    }

}

impl OverlayRenderer for Canvas {
    fn init(&mut self, sk: &stereokit::SkDraw, app: &mut AppState) {
        let gl = CanvasGl{
            tex_bg: sk.tex_create(TextureType::IMAGE_NO_MIPS, stereokit::TextureFormat::RGBA32),
            tex_hl: sk.tex_create(TextureType::IMAGE_NO_MIPS, stereokit::TextureFormat::RGBA32),
            tex_fg: sk.tex_create(TextureType::IMAGE_NO_MIPS, stereokit::TextureFormat::RGBA32),
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
    fn pause(&mut self, _app: &mut AppState) { }
    fn resume(&mut self, _app: &mut AppState) { }
    fn render(&mut self, sk: &stereokit::SkDraw, tex: &Tex, app: &mut AppState) {
        if self.gl.is_none() {
            return;
        }
        let gl = self.gl.as_ref().unwrap();

        app.gl.begin_sk(sk, tex);

        // static background
        let handle = unsafe { sk.tex_get_surface(&gl.tex_bg.as_ref()) } as usize as u32;
        app.gl.draw_sprite_full(handle);

        // interactive highlights etc
        let handle = unsafe { sk.tex_get_surface(&gl.tex_hl.as_ref()) } as usize as u32;
        app.gl.draw_sprite_full(handle);

        // mostly static text
        let handle = unsafe { sk.tex_get_surface(&gl.tex_fg.as_ref()) } as usize as u32;
        app.gl.draw_sprite_full(handle);
        
        app.gl.end();
    }
}

struct Control {
    rect: Rect,
    color: Vec3,
    text: String,
    size: isize,
    dirty: bool,

    on_update: Option<fn(&mut Self, &mut Canvas, &mut AppState) -> bool>,
    on_render_bg: Option<fn(&mut Self, &SkDraw, &mut AppState)>,
    on_render_hl: Option<fn(&mut Self, &SkDraw, &mut AppState, bool)>,
    on_render_fg: Option<fn(&mut Self, &SkDraw, &mut AppState)>,
}

impl Default for Control {
    fn default() -> Self {
        Self { 
            rect: Rect{ x: 0., y: 0., w: 0., h: 0. },
            color: Vec3{ x: 1., y: 1., z: 1. },
            text: String::new(), 
            dirty: false,
            size: 24,
            on_update: None, 
            on_render_bg: None, 
            on_render_hl: None, 
            on_render_fg: None,
        }
    }
}

impl Control {
    fn render_rect(&mut self, _sk: &SkDraw, app: &mut AppState) {
        app.gl.draw_color(self.color, self.rect.x, self.rect.y, self.rect.w, self.rect.h);
    }
    fn render_text(&mut self, sk: &SkDraw, app: &mut AppState) {
        let mut cur_y = self.rect.y;
        for line in self.text.lines() {
            
            let mut cur_x = self.rect.x;
            for glyph in app.fc.get_glyphs(line, self.size, sk) {
                if let Some(tex) = &glyph.tex {
                    let handle = unsafe { sk.tex_get_surface(&tex.as_ref()) } as usize as u32;
                    app.gl.draw_glyph(handle, cur_x + glyph.left, cur_y + glyph.top, glyph.width, glyph.height);
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
                    let handle = unsafe { sk.tex_get_surface(&tex.as_ref()) } as usize as u32;
                    app.gl.draw_glyph(handle, cur_x + glyph.left, cur_y + glyph.top, glyph.width, glyph.height);
                }

                cur_x += glyph.advance;
            }
            cur_y += (self.size as f32) * 1.5;
        }
    }
}

