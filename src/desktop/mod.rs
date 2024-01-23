use std::{
    f32::consts::PI,
    fs::read_to_string,
    path::Path,
    time::{Duration, Instant},
};

use glam::{vec2, Affine2, Quat, Vec2, Vec3};
use log::{info, warn};
use wayland_client::protocol::wl_output::Transform;

use crate::{
    desktop::capture::{
        pw_capture::{pipewire_select_screen, PipewireCapture},
        wlr_dmabuf_capture::WlrDmabufCapture,
    },
    input::{INPUT, MOUSE_LEFT, MOUSE_MIDDLE, MOUSE_RIGHT},
    interactions::{InteractionHandler, PointerHit, POINTER_ALT, POINTER_SHIFT},
    overlay::{OverlayData, OverlayRenderer, SplitOverlayBackend},
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
    fn new(pos: Vec2, size: Vec2, transform: Transform) -> ScreenInteractionHandler {
        let transform = match transform {
            Transform::_90 | Transform::Flipped90 => Affine2::from_cols(
                vec2(0., size.y),
                vec2(-size.x, 0.),
                vec2(pos.x + size.x, pos.y),
            ),
            Transform::_180 | Transform::Flipped180 => Affine2::from_cols(
                vec2(-size.x, 0.),
                vec2(0., -size.y),
                vec2(pos.x + size.x, pos.y + size.y),
            ),
            Transform::_270 | Transform::Flipped270 => Affine2::from_cols(
                vec2(0., -size.y),
                vec2(size.x, 0.),
                vec2(pos.x, pos.y + size.y),
            ),
            _ => Affine2::from_cols(vec2(size.x, 0.), vec2(0., size.y), pos),
        };

        ScreenInteractionHandler {
            next_scroll: Instant::now(),
            next_move: Instant::now(),
            mouse_transform: transform,
        }
    }
}

impl InteractionHandler for ScreenInteractionHandler {
    fn on_hover(&mut self, hit: &PointerHit) {
        if self.next_move < Instant::now() {
            if let Ok(mut input) = INPUT.lock() {
                let pos = self.mouse_transform.transform_point2(hit.uv);
                input.mouse_move(pos);
            }
        }
    }
    fn on_pointer(&mut self, hit: &PointerHit, pressed: bool) {
        if let Ok(mut input) = INPUT.lock() {
            let pos = self.mouse_transform.transform_point2(hit.uv);
            input.mouse_move(pos);

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
}

pub async fn try_create_screen(
    wl: &WlClientState,
    idx: usize,
    session: &AppSession,
) -> Option<OverlayData> {
    let output = &wl.outputs[idx];
    info!(
        "{}: Res {}x{} Size {:?} Pos {:?}",
        output.name, output.size.0, output.size.1, output.logical_size, output.logical_pos,
    );

    let size = (output.size.0, output.size.1);
    let mut capture: Option<Box<dyn OverlayRenderer>> = None;

    if session.capture_method == "auto" && wl.maybe_wlr_dmabuf_mgr.is_some() {
        info!("{}: Using Wlr DMA-Buf", &output.name);
        let wl = WlClientState::new();
        capture = WlrDmabufCapture::try_new(wl, output);
    } else {
        info!("{}: Using Pipewire capture", &output.name);
        let file_name = format!("{}.token", &output.name);
        let full_path = Path::new(&session.config_root_path).join(file_name);
        let token = read_to_string(full_path).ok();

        if let Ok(node_id) = pipewire_select_screen(token.as_deref()).await {
            info!("Node id: {}", node_id);
            capture = Some(Box::new(PipewireCapture::new(
                output.name.clone(),
                node_id,
                60,
                session.capture_method != "pw-fallback",
            )));
        }
    }
    if let Some(capture) = capture {
        let backend = Box::new(SplitOverlayBackend {
            renderer: capture,
            interaction: Box::new(ScreenInteractionHandler::new(
                output.logical_pos,
                output.logical_size,
                output.transform,
            )),
        });

        let axis = Vec3::new(0., 0., 1.);

        let angle = match output.transform {
            Transform::_90 | Transform::Flipped90 => PI / 2.,
            Transform::_180 | Transform::Flipped180 => PI,
            Transform::_270 | Transform::Flipped270 => -PI / 2.,
            _ => 0.,
        };

        Some(OverlayData {
            name: output.name.clone(),
            size,
            show_hide: true,
            grabbable: true,
            backend,
            spawn_rotation: Quat::from_axis_angle(axis, angle),
            ..Default::default()
        })
    } else {
        warn!("{}: Will not be used", &output.name);
        None
    }
}
