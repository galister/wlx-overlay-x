use std::time::Instant;

use chrono::Local;
use glam::{Quat, Vec3};

use crate::{
    gui::{color_parse, Canvas},
    overlay::{OverlayData, RelativeTo},
    AppSession, TASKS,
};

pub const WATCH_DEFAULT_POS: Vec3 = Vec3::new(0., 0., 0.15);
pub const WATCH_DEFAULT_ROT: Quat = Quat::from_xyzw(0.7071066, 0., 0.7071066, 0.0007963);

pub fn create_watch(session: &AppSession, screens: Vec<(usize, String)>) -> OverlayData {
    let mut canvas = Canvas::new(400, 200, ());

    // Background
    canvas.bg_color = color_parse("#353535");
    canvas.panel(0., 0., 400., 200.);

    // Time display
    canvas.font_size = 46;
    let clock = canvas.label(19., 100., 200., 50., String::new());
    canvas.controls[clock].on_update = Some(|control, _data| {
        let date = Local::now();
        control.set_text(format!("{}", &date.format("%H:%M")));
    });

    canvas.font_size = 14;
    let date = canvas.label(20., 125., 200., 50., String::new());
    canvas.controls[date].on_update = Some(|control, _data| {
        let date = Local::now();
        control.set_text(format!("{}", &date.format("%x")));
    });

    let day_of_week = canvas.label(20., 150., 200., 50., String::new());
    canvas.controls[day_of_week].on_update = Some(|control, _data| {
        let date = Local::now();
        control.set_text(format!("{}", &date.format("%A")));
    });

    // Volume controls
    canvas.bg_color = color_parse("#222222");
    canvas.fg_color = color_parse("#AAAAAA");
    canvas.font_size = 14;

    canvas.bg_color = color_parse("#303030");
    canvas.fg_color = color_parse("#353535");

    let vol_up = canvas.button(327., 116., 46., 32., String::from("+"));
    canvas.controls[vol_up].on_press = Some(|_control, _data| {
        println!("Volume up!"); //TODO
    });

    let vol_dn = canvas.button(327., 52., 46., 32., String::from("-"));
    canvas.controls[vol_dn].on_press = Some(|_control, _data| {
        println!("Volume down!"); //TODO
    });

    canvas.bg_color = color_parse("#303030");
    canvas.fg_color = color_parse("#353535");

    let settings = canvas.button(2., 162., 36., 36., "☰".to_string());
    canvas.controls[settings].on_press = Some(|_control, _data| {
        println!("Settings!"); //TODO
    });

    canvas.fg_color = color_parse("#CCBBAA");
    canvas.bg_color = color_parse("#406050");
    // Bottom row
    let num_buttons = screens.len() + 1;
    let button_width = 360. / num_buttons as f32;
    let mut button_x = 40.;

    let i = canvas.button(button_x + 2., 162., button_width - 4., 36., "Kbd".to_string());
    let keyboard = &mut canvas.controls[i];
    keyboard.state = Some(WatchButtonState {
        pressed_at: Instant::now(),
        scr_idx: 0,
    });

    keyboard.on_press = Some(|control, _data| {
        if let Some(state) = control.state.as_mut() {
            state.pressed_at = Instant::now();
        }
    });
    keyboard.on_release = Some(|control, _data| {
        if let Some(state) = control.state.as_ref() {
            if let Ok(mut tasks) = TASKS.lock() {
                if Instant::now()
                    .saturating_duration_since(state.pressed_at)
                    .as_millis()
                    < 2000
                {
                    tasks.push_back(Box::new(|_sk, _app, o| {
                        for overlay in o {
                            if overlay.name == "Kbd" {
                                overlay.want_visible = !overlay.want_visible;
                                return;
                            }
                        }
                    }));
                } else {
                    tasks.push_back(Box::new(|_sk, app, o| {
                        for overlay in o {
                            if overlay.name == "Kbd" {
                                overlay.reset(app);
                            }
                        }
                    }));
                }
            }
        }
    });
    button_x += button_width;

    canvas.bg_color = color_parse("#405060");

    for (scr_idx, scr_name) in screens.into_iter() {
        let i = canvas.button(button_x + 2., 162., button_width - 4., 36., scr_name);
        let button = &mut canvas.controls[i];
        button.state = Some(WatchButtonState {
            pressed_at: Instant::now(),
            scr_idx,
        });

        button.on_press = Some(|control, _data| {
            if let Some(state) = control.state.as_mut() {
                state.pressed_at = Instant::now();
            }
        });
        button.on_release = Some(|control, _data| {
            if let Some(state) = control.state.as_ref() {
                if let Ok(mut tasks) = TASKS.lock() {
                    let scr_idx = state.scr_idx;
                    if Instant::now()
                        .saturating_duration_since(state.pressed_at)
                        .as_millis()
                        < 2000
                    {
                        tasks.push_back(Box::new(move |_sk, _app, o| {
                            o[scr_idx].want_visible = !o[scr_idx].want_visible;
                        }));
                    } else {
                        tasks.push_back(Box::new(move |_sk, app, o| {
                            o[scr_idx].reset(app);
                        }));
                    }
                }
            }
        });
        button_x += button_width;
    }

    let relative_to = RelativeTo::Hand(session.watch_hand);

    OverlayData {
        name: "Watch".to_string(),
        size: (400, 200),
        width: 0.065,
        backend: Box::new(canvas),
        want_visible: true,
        relative_to,
        spawn_point: session.watch_pos,
        spawn_rotation: session.watch_rot,
        ..Default::default()
    }
}

struct WatchButtonState {
    pressed_at: Instant,
    scr_idx: usize,
}
