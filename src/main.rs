use std::{fs::create_dir, path::Path};

use desktop::{try_create_screen, wl_client::WlClientState};
use gl::{egl::gl_init, GlRenderer, PANEL_SHADER_BYTES};
use glam::{Quat, Vec3};
use gui::font::FontCache;
use input::INPUT;
use interactions::InputState;
use log::error;
use once_cell::sync::Lazy;
use overlay::OverlayData;
use stereokit::*;
use tokio::runtime::{Builder, Runtime};
use watch::{WatchPanel, WATCH_DEFAULT_POS, WATCH_DEFAULT_ROT};

mod desktop;
mod gl;
mod gui;
mod input;
mod interactions;
mod overlay;
mod watch;

pub struct AppSession {
    pub config_path: String,

    pub screen_flip_h: bool,
    pub screen_flip_v: bool,
    pub screen_invert_color: bool,

    pub watch_hand: u32,
    pub watch_pos: Vec3,
    pub watch_rot: Quat,

    pub primary_hand: usize,

    pub color_norm: Color32,
    pub color_shift: Color32,
    pub color_alt: Color32,
    pub color_grab: Color32,
}

impl AppSession {
    pub fn load() -> AppSession {
        let config_path: String;

        if let Ok(home) = std::env::var("HOME") {
            config_path = Path::new(&home)
                .join(".config/wlxroverlay")
                .to_str()
                .unwrap()
                .to_string();
        } else {
            config_path = "/tmp/wlxroverlay".to_string();
            error!("Err: $HOME is not set, using {}", config_path);
        }
        let _ = create_dir(&config_path);

        AppSession {
            config_path,
            screen_flip_h: false,
            screen_flip_v: false,
            screen_invert_color: false,
            primary_hand: 1,
            watch_hand: 0,
            watch_pos: WATCH_DEFAULT_POS,
            watch_rot: WATCH_DEFAULT_ROT,
            color_norm: Color32 {
                r: 255,
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
                r: 0,
                g: 255,
                b: 255,
                a: 255,
            },
            color_grab: Color32 {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
        }
    }
}

pub struct AppState {
    gl: GlRenderer,
    input: InputState,
    session: AppSession,
    rt: Runtime,
    fc: FontCache,
    panel_shader: Shader,
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

    let wl = WlClientState::new();
    if let Ok(mut input) = INPUT.lock() {
        input.set_desktop_extent(wl.get_desktop_extent());
    }

    for i in 0..wl.outputs.len() {
        let want_visible = wl.outputs[i].name == "DP-3";
        let maybe_screen = rt.block_on(try_create_screen(&wl, i, &session));
        if let Some(mut screen) = maybe_screen {
            screen.want_visible = want_visible;
            overlays.push(screen);
        }
    }

    let mut watch = WatchPanel::new(&session);

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
            app.input.update(sk, overlays.as_mut_slice());

            watch.render(sk);

            for screen in overlays.iter_mut() {
                if screen.want_visible && !screen.visible {
                    screen.show(sk, &mut app);
                }

                screen.render(sk, &mut app);
            }

            if let Ok(mut input) = INPUT.lock() {
                input.on_new_frame();
            }
        },
        |_| {},
    );
}
