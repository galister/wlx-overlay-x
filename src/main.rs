use desktop::{screen::DesktopScreen, wl_client::WlClientState};
use gl::{egl::gl_init, GlRenderer};
use once_cell::unsync::Lazy;
use overlay::Overlay;
use stereokit::*;

mod desktop;
mod gl;
mod overlay;
mod session;

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

    gl_init(&sk);

    let mut screens: Vec<DesktopScreen> = vec![];

    let wl = WlClientState::new();
    let mut first = true;
    for output in wl.outputs {
        let mut screen = DesktopScreen::new(output);
        if screen.try_init(wl.maybe_wlr_dmabuf_mgr.is_some()).await {
            if first {
                screen.overlay_mut().want_visible = true;
                first = false;
            }
            screens.push(screen);
        }
    }

    let mut state = Lazy::new(|| AppState {
        renderer: GlRenderer::new(),
    });

    sk.run(
        |sk| {
            for screen in screens.iter_mut() {
                let overlay = screen.overlay_mut();
                if overlay.want_visible && !overlay.visible {
                    screen.show(sk);
                }

                screen.render(sk, &mut state);
            }
        },
        |_| {},
    );
}
