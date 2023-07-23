use std::time::Instant;

use desktop::{screen::DesktopScreen, wl_client::WlClientState};
use gl::{egl::gl_init, GlRenderer};
use once_cell::unsync::Lazy;
use overlay::Overlay;
use signal::{Signal, trap::Trap};
use stereokit::*;
use watch::WatchPanel;

mod desktop;
mod gl;
mod overlay;
mod session;
mod watch;

pub struct AppState {
    renderer: GlRenderer,
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
    
    let trap = Trap::trap(&vec![Signal::SIGBUS]);


    gl_init(&sk);

    let mut screens: Vec<DesktopScreen> = vec![];

    let wl = WlClientState::new();
    for output in wl.outputs {
        let want_visible = output.name == "DP-3";
        let mut screen = DesktopScreen::new(output);
        if screen.try_init(wl.maybe_wlr_dmabuf_mgr.is_some()).await {
            screen.overlay_mut().want_visible = want_visible;
            screens.push(screen);
        }
    }

    let mut state = Lazy::new(|| AppState {
        renderer: GlRenderer::new(),
    });

    let mut watch = WatchPanel::new();

    sk.run(
        |sk| {

            watch.render(sk, &mut state);

            for screen in screens.iter_mut() {
                let overlay = screen.overlay_mut();
                if overlay.want_visible && !overlay.visible {
                    screen.show(sk);
                }

                screen.render(sk, &mut state);
                if trap.wait(Instant::now()).is_some() {
                    println!("SIGBUS caught!");
                }
            }
        },
        |_| {},
    );
}
