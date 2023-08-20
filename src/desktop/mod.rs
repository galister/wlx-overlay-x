use std::{
    fs::read_to_string,
    path::Path,
    time::{Duration, Instant},
};

use glam::{vec2, Affine2};
use log::{info, warn};
use stereokit::SkDraw;

use crate::{
    desktop::capture::{
        pw_capture::{pipewire_select_screen, PipewireCapture},
        wlr_dmabuf_capture::WlrDmabufCapture,
    },
    input::{INPUT, MOUSE_LEFT, MOUSE_MIDDLE, MOUSE_RIGHT},
    interactions::{InteractionHandler, PointerHit, POINTER_ALT, POINTER_SHIFT, InputState},
    overlay::{OverlayData, OverlayRenderer},
    AppSession,
};

use self::wl_client::WlClientState;

pub mod capture;
pub mod frame;
pub mod wl_client;

struct ScreenInteractionHandler {
    next_scroll: Instant,
    next_move: Instant,
    mouse_transform: Affine2,
}

impl ScreenInteractionHandler {
    fn new(pos: (i32, i32), size: (i32, i32)) -> ScreenInteractionHandler {
        ScreenInteractionHandler {
            next_scroll: Instant::now(),
            next_move: Instant::now(),
            mouse_transform: Affine2::from_scale_angle_translation(
                vec2(size.0 as _, size.1 as _),
                0.,
                vec2(pos.0 as _, pos.1 as _),
            ),
        }
    }
}

impl InteractionHandler for ScreenInteractionHandler {
    fn on_hover(&mut self, hit: &PointerHit) {
        if self.next_move < Instant::now() {
            if let Ok(mut input) = INPUT.lock() {
                let xy = self.mouse_transform.transform_point2(hit.uv);
                input.mouse_move(xy.x as _, xy.y as _);
            }
        }
    }
    fn on_pointer(&mut self, hit: &PointerHit, pressed: bool) {
        if let Ok(mut input) = INPUT.lock() {
            let xy = self.mouse_transform.transform_point2(hit.uv);
            input.mouse_move(xy.x as _, xy.y as _);

            let btn = match hit.mode {
                POINTER_SHIFT => MOUSE_RIGHT,
                POINTER_ALT => MOUSE_MIDDLE,
                _ => MOUSE_LEFT,
            };

            if pressed {
                self.next_move = Instant::now() + Duration::from_millis(300);
            }

            input.send_button(btn, pressed);
        }
    }
    fn on_scroll(&mut self, _hit: &PointerHit, delta: f32) {
        if let Ok(input) = INPUT.lock() {
            let millis = (1. - delta.abs()) * delta;
            if let Some(next_scroll) =
                Instant::now().checked_add(Duration::from_millis(millis as _))
            {
                self.next_scroll = next_scroll;
            }
            input.wheel(if delta < 0. { -1 } else { 1 })
        }
    }
    fn on_left(&mut self, _hand: usize) {}
    fn on_pose_updated(&mut self, _input: &InputState, _sk: &SkDraw) {}
    fn on_interactions_done(&mut self, _input: &InputState, _sk: &SkDraw) {}
}

pub async fn try_create_screen(
    wl: &WlClientState,
    idx: usize,
    session: &AppSession,
) -> Option<OverlayData> {
    let output = &wl.outputs[idx];
    info!(
        "{}: Res {}x{} Size {}x{} Pos {}x{}",
        output.name,
        output.size.0,
        output.size.1,
        output.logical_size.0,
        output.logical_size.1,
        output.logical_pos.0,
        output.logical_pos.1
    );

    let size = (output.size.0, output.size.1);
    let mut capture: Option<Box<dyn OverlayRenderer>> = None;

    if wl.maybe_wlr_dmabuf_mgr.is_some() {
        info!("{}: Using Wlr DMA-Buf", &output.name);
        let wl = WlClientState::new();
        capture = WlrDmabufCapture::try_new(wl, output);
    } else {
        info!("{}: Using Pipewire capture", &output.name);
        let file_name = format!("{}.token", &output.name);
        let full_path = Path::new(&session.config_path).join(file_name);
        let token = read_to_string(full_path).ok();

        if let Ok(node_id) = pipewire_select_screen(token.as_deref()).await {
            info!("Node id: {}", node_id);
            capture = Some(Box::new(PipewireCapture::new(
                output.name.clone(),
                node_id,
                60,
                true,
            )));
        }
    }
    if let Some(capture) = capture {
        Some(OverlayData {
            name: output.name.clone(),
            size,
            grabbable: true,
            renderer: capture,
            interaction: Box::new(ScreenInteractionHandler::new(
                output.logical_pos,
                output.logical_size,
            )),
            ..Default::default()
        })
    } else {
        warn!("{}: Will not be used", &output.name);
        None
    }
}
