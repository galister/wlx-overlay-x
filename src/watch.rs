use std::{cell::RefCell, mem::transmute, rc::Rc};

use chrono::{Local, Utc};
use glam::{Affine3A, Quat, Vec3};
use stereokit::StereoKitMultiThread; 

use crate::{
    gui::{color_parse, Canvas},
    AppSession, AppState, interactions::InteractionHandler, overlay::{OverlayRenderer, OverlayData},
};

pub const WATCH_DEFAULT_POS: Vec3 = Vec3::new(0., -0.05, 0.05);
pub const WATCH_DEFAULT_ROT: Quat = Quat::from_xyzw(0., 1., 0., 0.);

    pub fn create_watch(session: &AppSession, screens: Vec<(usize, String)>) -> OverlayData {
        let mut canvas = Canvas::<WatchInteractResult>::new(400, 200);

        // Background
        canvas.bg_color = color_parse("#353535");
        canvas.panel(0., 0., 400., 200.);

        // Time display
        canvas.font_size = 46;
        let clock = canvas.label(19., 107., 200., 50., String::new());
        canvas.controls[clock].on_update = Some(|control| {
            let date = Local::now();
            control.set_text(format!("{}", &date.format("%H:%M")));
        });

        canvas.font_size = 14;
        let date = canvas.label(20., 80., 200., 50., String::new());
        canvas.controls[date].on_update = Some(|control| {
            let date = Local::now();
            control.set_text(format!("{}", &date.format("%x")));
        });

        let day_of_week = canvas.label(20., 60., 200., 50., String::new());
        canvas.controls[day_of_week].on_update = Some(|control| {
            let date = Local::now();
            control.set_text(format!("{}", &date.format("%A")));
        });

        // Volume controls
        canvas.bg_color = color_parse("#222222");
        canvas.fg_color = color_parse("#AAAAAA");
        canvas.font_size = 14;

        let vol_up = canvas.button(327., 116., 46., 32., String::from("+"));
        canvas.controls[vol_up].on_press = Some(|_control| {
            println!("Volume up!");
            Some(WatchInteractResult::VolumeUp)
        });

        let vol_dn = canvas.button(327., 52., 46., 32., String::from("-"));
        canvas.controls[vol_dn].on_press = Some(|_control| {
            println!("Volume down!");
            Some(WatchInteractResult::VolumeDown)
        });

        canvas.bg_color = color_parse("#406050");
        canvas.fg_color = color_parse("#CCBBAA");

        let settings = canvas.button(2., 2., 36., 36., "☰".to_string());
        canvas.controls[settings].on_press = Some(|_control| {
            println!("Settings!");
            None
        });

        let num_buttons = screens.len() + 1;
        let button_width = 400. / num_buttons as f32;
        let mut button_x = 40.;

        let i = canvas.button(button_x + 2., 2., button_width - 4., 36., "⌨".to_string());
        let keyboard = &mut canvas.controls[i];
        keyboard.data = vec![0];
        keyboard.on_press = Some(|control| {
            control.data[0] = Utc::now().timestamp_millis() as _;
            None
        });
        keyboard.on_release = Some(|control| {
            let now = Utc::now().timestamp_millis() as usize;
            let pressed_at = control.data[0];
            if now - pressed_at < 2000 {
                Some(WatchInteractResult::ToggleKeyboard)
            } else {
                Some(WatchInteractResult::ResetKeyboard)
            }
        });
        button_x += button_width;

        for (scr_idx, scr_name) in screens.into_iter() {
            let i = canvas.button(button_x + 2., 2., button_width - 4., 36., scr_name);
            let button = &mut canvas.controls[i];
            button.data = vec![0, scr_idx];
            button.on_press = Some(|control| {
                control.data[0] = Utc::now().timestamp_millis() as _;
                None
            });
            button.on_release = Some(|control| {
                let now = Utc::now().timestamp_millis() as usize;
                let pressed_at = control.data[0];
                let scr_idx = control.data[1];
                if now - pressed_at < 2000 {
                    Some(WatchInteractResult::ToggleScreen(scr_idx))
                } else {
                    Some(WatchInteractResult::ResetScreen(scr_idx))
                }
            });
            button_x += button_width;
        }

    let canvas = Rc::new(RefCell::new(canvas));

    let interaction = Box::new(WatchInteraction {
        canvas: canvas.clone(),
        hand: session.watch_hand,
    });

    let renderer = Box::new(WatchRenderer {
        canvas,
    });

        OverlayData {
            name: "Watch".to_string(),
            size: (400, 200),
            width: 0.115,
            renderer,
            interaction,
            transform: Affine3A::from_rotation_translation(session.watch_rot, session.watch_pos),
            ..Default::default()
        }
    }

enum WatchInteractResult {
    ToggleKeyboard,
    ResetKeyboard,
    ToggleScreen(usize),
    ResetScreen(usize),
    VolumeUp,
    VolumeDown,
}

struct WatchInteraction {
    canvas: Rc<RefCell<Canvas<WatchInteractResult>>>,
    hand: u32,
}

impl WatchInteraction {
    fn handle_result(&mut self, result: Option<WatchInteractResult>) {
        match result {
            Some(WatchInteractResult::ToggleKeyboard) => {
                println!("Toggle Keyboard");
            }
            Some(WatchInteractResult::ResetKeyboard) => {
                println!("Reset Keyboard");
            }
            Some(WatchInteractResult::ToggleScreen(idx)) => {
                println!("Toggle Screen {}", idx);
            }
            Some(WatchInteractResult::ResetScreen(idx)) => {
                println!("Reset Screen {}", idx);
            }
            Some(WatchInteractResult::VolumeUp) => {
                println!("Volume Up");
            }
            Some(WatchInteractResult::VolumeDown) => {
                println!("Volume Down");
            }
            None => {}
        }
    }
}

impl InteractionHandler for WatchInteraction {
    fn on_left(&mut self, hand: usize) {
        self.canvas.borrow_mut().on_left(hand);
    }
    fn on_hover(&mut self, hit: &crate::interactions::PointerHit) {
        self.canvas.borrow_mut().on_hover(hit);
    }
    fn on_scroll(&mut self, hit: &crate::interactions::PointerHit, delta: f32) {
        let result = self.canvas.borrow_mut().on_scroll(hit, delta);
        self.handle_result(result);
    }
    fn on_pointer(&mut self, hit: &crate::interactions::PointerHit, pressed: bool) {
        let result = self.canvas.borrow_mut().on_pointer(hit, pressed);
        self.handle_result(result);
    }
    fn on_pose_updated(&mut self, _input: &crate::interactions::InputState, sk: &stereokit::SkDraw) {
        let cur_hand = sk.input_hand(unsafe { transmute(self.hand) });
        let mat = Affine3A::from_rotation_translation(cur_hand.palm.orientation, cur_hand.palm.position);
    }
    fn on_interactions_done(&mut self, _input: &crate::interactions::InputState, sk: &stereokit::SkDraw) {
    }
}

struct WatchRenderer {
    canvas: Rc<RefCell<Canvas<WatchInteractResult>>>,
}

impl OverlayRenderer for WatchRenderer {
    fn init(&mut self, sk: &stereokit::SkDraw, app: &mut AppState) {
        self.canvas.borrow_mut().init(sk, app);
    }
    fn pause(&mut self, app: &mut AppState) {
        self.canvas.borrow_mut().pause(app);
    }
    fn resume(&mut self, app: &mut AppState) {
       self.canvas.borrow_mut().resume(app); 
    }
    fn render(&mut self, sk: &stereokit::SkDraw, tex: &stereokit::Tex, app: &mut AppState) {
       self.canvas.borrow_mut().render(sk, tex, app); 
    }
}

