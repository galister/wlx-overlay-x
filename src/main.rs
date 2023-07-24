use std::{fs::create_dir, path::Path, sync::Mutex};

use desktop::{wl_client::WlClientState, maybe_create_screen};
use gl::{egl::gl_init, GlRenderer};
use glam::{Quat, Vec3};
use input::INPUT;
use interactions::InputState;
use log::error;
use once_cell::sync::Lazy;
use overlay::OverlayData;
use stereokit::*;
use watch::{WatchPanel, WATCH_DEFAULT_POS, WATCH_DEFAULT_ROT};

mod desktop;
mod gl;
mod input;
mod interactions;
mod overlay;
mod session;
mod watch;

pub static SESSION: Lazy<Mutex<WlXrSession>> = Lazy::new(|| Mutex::new(WlXrSession::load()));

pub struct WlXrSession {
    pub config_path: String,

    pub screen_flip_h: bool,
    pub screen_flip_v: bool,
    pub screen_invert_color: bool,

    pub watch_hand: u32,
    pub watch_pos: Vec3,
    pub watch_rot: Quat,
}

impl WlXrSession {
    pub fn load() -> WlXrSession {
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

        WlXrSession {
            config_path,
            screen_flip_h: false,
            screen_flip_v: false,
            screen_invert_color: false,
            watch_hand: 0,
            watch_pos: WATCH_DEFAULT_POS,
            watch_rot: WATCH_DEFAULT_ROT,
        }
    }
}

pub struct AppState {
    gl: GlRenderer,
    input: InputState,
}

#[tokio::main]
async fn main() {
    let sk = stereokit::Settings {
        app_name: "WlXrOverlay".to_string(),
        display_preference: DisplayMode::MixedReality,
        blend_preference: DisplayBlend::AnyTransparent,
        depth_mode: DepthMode::D32,
        overlay_app: false,
        overlay_priority: 1u32,
        disable_desktop_input_window: true,
        ..Default::default()
    }
    .init()
    .expect("StereoKit init fail!");

    env_logger::init();

    gl_init(&sk);

    let mut overlays: Vec<OverlayData> = vec![];

    let wl = WlClientState::new();
    if let Ok(mut input) = INPUT.lock() {
        input.set_desktop_extent(wl.get_desktop_extent());
    }

    for i in 0..wl.outputs.len() {
        let want_visible = wl.outputs[i].name == "DP-1";
        if let Some(mut screen) = maybe_create_screen(&wl, i).await {
            screen.want_visible = want_visible;
            overlays.push(screen);
        }
    }

    let mut app = Lazy::new(|| AppState {
        gl: GlRenderer::new(),
        input: InputState::new(),
    });

    let mut watch = WatchPanel::new();

    sk.run(
        |sk| {
            app.input.update(sk, overlays.as_mut_slice());

            watch.render(sk);

            for screen in overlays.iter_mut() {
                if screen.want_visible && !screen.visible {
                    screen.show(sk);
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
