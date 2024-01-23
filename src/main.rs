#![allow(dead_code)]
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use desktop::{try_create_screen, wl_client::WlClientState};
use gl::{egl::gl_init, GlRenderer, PANEL_SHADER_BYTES};
use glam::{Quat, Vec3};
use gui::font::FontCache;
use input::INPUT;
use interactions::InputState;
use keyboard::create_keyboard;
use once_cell::sync::Lazy;
use overlay::OverlayData;
use stereokit::*;
use tokio::runtime::{Builder, Runtime};
use watch::{create_watch, WATCH_DEFAULT_POS, WATCH_DEFAULT_ROT};

mod config;
mod desktop;
mod gl;
mod gui;
mod input;
mod interactions;
mod keyboard;
mod overlay;
mod watch;

pub type Task = Box<dyn FnOnce(&SkDraw, &mut AppState, &mut [OverlayData]) + Send>;
pub static TASKS: Lazy<Mutex<VecDeque<Task>>> = Lazy::new(|| Mutex::new(VecDeque::new()));

pub struct AppSession {
    pub config_root_path: PathBuf,

    pub show_screens: Vec<String>,
    pub show_keyboard: bool,
    pub keyboard_volume: f32,

    pub screen_flip_h: bool,
    pub screen_flip_v: bool,
    pub screen_invert_color: bool,

    pub watch_hand: usize,
    pub watch_pos: Vec3,
    pub watch_rot: Quat,

    pub primary_hand: usize,

    pub capture_method: String,

    pub color_norm: Color32,
    pub color_shift: Color32,
    pub color_alt: Color32,
    pub color_grab: Color32,

    pub click_freeze_time_ms: u64,
}

impl AppSession {
    pub fn load() -> AppSession {
        let config_root_path = config::ensure_config_root();
        println!("Config root path: {}", config_root_path.to_string_lossy());

        AppSession {
            config_root_path,
            show_screens: vec!["DP-3".to_string()],
            keyboard_volume: 0.5,
            show_keyboard: false,
            screen_flip_h: false,
            screen_flip_v: false,
            screen_invert_color: false,
            capture_method: "auto".to_string(),
            primary_hand: 1,
            watch_hand: 1,
            watch_pos: WATCH_DEFAULT_POS,
            watch_rot: WATCH_DEFAULT_ROT,
            color_norm: Color32 {
                r: 0,
                g: 255,
                b: 255,
                a: 255,
            },
            color_shift: Color32 {
                r: 255,
                g: 255,
                b: 0,
                a: 255,
            },
            color_alt: Color32 {
                r: 255,
                g: 0,
                b: 255,
                a: 255,
            },
            color_grab: Color32 {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            click_freeze_time_ms: 300,
        }
    }
}

// Contains runtime resources
pub struct AppState {
    fc: FontCache,
    gl: GlRenderer,
    input: InputState,
    panel_shader: Shader,
    rt: Runtime,
    session: AppSession,
}

impl AppState {
    fn update_input(&mut self, sk: &SkDraw, interactables: &mut [OverlayData]) {
        self.input.update(&self.session, sk, interactables);
    }
}

fn main() {
    let sk = stereokit::Settings {
        app_name: "WlXrOverlay".to_string(),
        display_preference: DisplayMode::MixedReality,
        blend_preference: DisplayBlend::AnyTransparent,
        depth_mode: DepthMode::D32,
        overlay_app: true,
        overlay_priority: 1u32,
        disable_desktop_input_window: true,
        ..Default::default()
    }
    .init()
    .expect("StereoKit init fail!");

    sk.input_hand_visible(Handed::Left, false);
    sk.input_hand_visible(Handed::Right, false);

    // disable built-in pointers
    unsafe {
        stereokit::sys::ui_enable_far_interact(0);
    };

    env_logger::init();

    let rt = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let session = AppSession::load();
    gl_init(&sk);

    let mut overlays: Vec<OverlayData> = vec![];
    let mut screens: Vec<(usize, Arc<str>)> = vec![];

    let wl = WlClientState::new();

    if let Ok(mut uinput) = INPUT.lock() {
        uinput.set_desktop_extent(wl.get_desktop_extent());
    }

    overlays.push(OverlayData::default()); // placeholder for watch

    let mut keyboard = create_keyboard();
    keyboard.want_visible = true;
    overlays.push(keyboard);

    for i in 0..wl.outputs.len() {
        let maybe_screen = rt.block_on(try_create_screen(&wl, i, &session));
        if let Some(mut screen) = maybe_screen {
            screen.want_visible = session.show_screens.iter().any(|s| s == &*screen.name);

            screens.push((overlays.len(), screen.name.clone()));
            overlays.push(screen);
        }
    }

    overlays[0] = create_watch(&session, screens);

    let panel_shader = sk.shader_create_mem(PANEL_SHADER_BYTES).unwrap();
    let mut app = Lazy::new(|| AppState {
        gl: GlRenderer::new(),
        input: InputState::new(&session),
        session,
        rt,
        fc: FontCache::new(),
        panel_shader,
    });

    sk.run(
        |sk| {
            app.update_input(sk, overlays.as_mut_slice());

            for overlay in overlays.iter_mut() {
                if overlay.want_visible && !overlay.visible {
                    overlay.show(sk, &mut app);
                } else if !overlay.want_visible && overlay.visible {
                    overlay.hide(&mut app);
                }

                overlay.render(sk, &mut app);
            }

            if let Ok(mut tasks) = TASKS.lock() {
                while let Some(task) = tasks.pop_front() {
                    task(sk, &mut app, overlays.as_mut_slice());
                }
            }

            if let Ok(mut uinput) = INPUT.lock() {
                uinput.on_new_frame();
            }
        },
        |_| {},
    );
}
