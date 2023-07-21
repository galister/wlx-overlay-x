use desktop::{screen::DesktopScreen, wl_client::WlClientState};
use gl::gl_init;
use stereokit::*;

mod desktop;
mod gl;
mod overlay;
mod session;

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
    for output in wl.outputs {
        let mut screen = DesktopScreen::new(output);
        if screen.try_init().await {
            screens.push(screen);
        }
    }

    sk.run(
        |sk| {
            for screen in &screens {
                screen.render(sk);
            }
        },
        |_| {},
    );
}
