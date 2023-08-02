use glam::Vec3;
use stereokit::Tex;

use crate::AppState;

pub mod font;

const RES_DIVIDER: i32 = 4;

struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

pub struct Canvas {
    pub width: i32,
    pub height: i32,
    controls: Vec<Control>,

    tex_bg: Tex,
    tex_hl: Tex,
    tex_fg: Tex,
}

impl Canvas {
    pub fn new() -> Canvas {
        todo!();
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

}



struct Control {
    rect: Rect,
    color: Vec3,
    text: String,
    dirty: bool,

    on_update: Option<fn(&mut Self, &mut Canvas, &mut AppState) -> bool>,
    on_render_bg: Option<fn(&mut Self, &mut Canvas, &mut AppState)>,
    on_render_hl: Option<fn(&mut Self, &mut Canvas, &mut AppState, bool)>,
    on_render_fg: Option<fn(&mut Self, &mut Canvas, &mut AppState)>,
}

impl Default for Control {
    fn default() -> Self {
        Self { 
            rect: Rect{ x: 0., y: 0., w: 0., h: 0. },
            color: Vec3{ x: 1., y: 1., z: 1. },
            text: String::new(), 
            dirty: false, 
            on_update: None, 
            on_render_bg: None, 
            on_render_hl: None, 
            on_render_fg: None,
        }
    }
}

impl Control {
    fn render_rect(&mut self, canvas: &mut Canvas, app: &mut AppState) {
        app.gl.draw_color(self.color, self.rect.x, self.rect.y, self.rect.w, self.rect.h);
    }
    fn render_text(&mut self, canvas: &mut Canvas, app: &mut AppState) {
        todo!();
    }
    fn render_text_centered(&mut self, canvas: &mut Canvas, app: &mut AppState) {
        
    }
}

