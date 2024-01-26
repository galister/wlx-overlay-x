use glam::{vec2, Vec2};
use log::warn;
use std::collections::BTreeMap;
use std::os::fd::IntoRawFd;
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::reexports::{
    protocols::xdg::xdg_output::zv1::client::{
        zxdg_output_manager_v1::ZxdgOutputManagerV1,
        zxdg_output_v1::{self, ZxdgOutputV1},
    },
    protocols_wlr::export_dmabuf::v1::client::{
        zwlr_export_dmabuf_frame_v1::{self, ZwlrExportDmabufFrameV1},
        zwlr_export_dmabuf_manager_v1::ZwlrExportDmabufManagerV1,
    },
};
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_output::{self, Transform, WlOutput},
        wl_registry::WlRegistry,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};

use crate::desktop::frame::{FramePlane, FRAME_FAILED};

use super::frame::{DmabufFrame, FRAME_PENDING, FRAME_READY};

pub struct OutputState {
    pub wl_output: WlOutput,
    pub id: u32,
    pub name: Arc<str>,
    pub model: Arc<str>,
    pub size: (i32, i32),
    pub logical_pos: Vec2,
    pub logical_size: Vec2,
    pub transform: Transform,
    done: bool,
}

pub struct WlClientState {
    pub connection: Arc<Connection>,
    pub xdg_output_mgr: ZxdgOutputManagerV1,
    pub maybe_wlr_dmabuf_mgr: Option<ZwlrExportDmabufManagerV1>,
    pub outputs: Vec<OutputState>,
    pub desktop_rect: (i32, i32),
    pub queue: Arc<Mutex<EventQueue<Self>>>,
    pub queue_handle: QueueHandle<Self>,
    pub pw_tokens: BTreeMap<String /* display name */, String /* token */>,
}

impl WlClientState {
    pub fn new() -> Self {
        let connection = Connection::connect_to_env().expect("wayland connection");
        let (globals, queue) = registry_queue_init::<Self>(&connection).expect("wayland globals");
        let qh = queue.handle();

        let mut state = Self {
            connection: Arc::new(connection),
            xdg_output_mgr: globals
                .bind(&qh, 2..=3, ())
                .expect(ZxdgOutputManagerV1::interface().name),
            maybe_wlr_dmabuf_mgr: globals.bind(&qh, 1..=1, ()).ok(),
            outputs: vec![],
            desktop_rect: (0, 0),
            queue: Arc::new(Mutex::new(queue)),
            queue_handle: qh.clone(),
            pw_tokens: BTreeMap::new(),
        };

        for o in globals.contents().clone_list().iter() {
            if o.interface == WlOutput::interface().name {
                let wl_output: WlOutput = globals.registry().bind(o.name, o.version, &qh, o.name);

                state.xdg_output_mgr.get_xdg_output(&wl_output, &qh, o.name);

                let unknown: Arc<str> = "Unknown".into();

                let output = OutputState {
                    wl_output,
                    id: o.name,
                    name: unknown.clone(),
                    model: unknown,
                    size: (0, 0),
                    logical_pos: Vec2::ZERO,
                    logical_size: Vec2::ZERO,
                    transform: Transform::Normal,
                    done: false,
                };

                state.outputs.push(output);
            }
        }

        state.dispatch();

        state
    }

    pub fn get_desktop_extent(&self) -> Vec2 {
        let mut extent = Vec2::ZERO;
        for output in self.outputs.iter() {
            extent.x = extent.x.max(output.logical_pos.x + output.logical_size.x);
            extent.y = extent.y.max(output.logical_pos.y + output.logical_size.y);
        }
        extent
    }

    pub fn request_dmabuf_frame(&mut self, output_idx: usize, frame: Arc<Mutex<DmabufFrame>>) {
        if let Some(dmabuf_manager) = self.maybe_wlr_dmabuf_mgr.as_ref() {
            let _ = dmabuf_manager.capture_output(
                1,
                &self.outputs[output_idx].wl_output,
                &self.queue_handle,
                frame,
            );

            self.dispatch();
        }
    }

    pub fn dispatch(&mut self) {
        if let Ok(mut queue_mut) = self.queue.clone().lock() {
            let _ = queue_mut.blocking_dispatch(self);
        }
    }
}

impl Dispatch<ZxdgOutputV1, u32> for WlClientState {
    fn event(
        state: &mut Self,
        _proxy: &ZxdgOutputV1,
        event: <ZxdgOutputV1 as Proxy>::Event,
        data: &u32,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zxdg_output_v1::Event::Name { name } => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    output.name = name.into();
                }
            }
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    output.logical_pos = vec2(x as _, y as _);
                }
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    output.logical_size = vec2(width as _, height as _);
                }
            }
            zxdg_output_v1::Event::Done => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    if output.logical_size.x < 0. {
                        output.logical_pos.x += output.logical_size.x;
                        output.logical_size.x *= -1.;
                    }
                    if output.logical_size.y < 0. {
                        output.logical_pos.y += output.logical_size.y;
                        output.logical_size.y *= -1.;
                    }
                    output.done = true;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, u32> for WlClientState {
    fn event(
        state: &mut Self,
        _proxy: &WlOutput,
        event: <WlOutput as Proxy>::Event,
        data: &u32,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Mode { width, height, .. } => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    output.size = (width, height);
                }
            }
            wl_output::Event::Geometry {
                model, transform, ..
            } => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    output.model = model.into();
                    output.transform = transform.into_result().unwrap_or(Transform::Normal);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrExportDmabufFrameV1, Arc<Mutex<DmabufFrame>>> for WlClientState {
    fn event(
        _state: &mut Self,
        proxy: &ZwlrExportDmabufFrameV1,
        event: <ZwlrExportDmabufFrameV1 as Proxy>::Event,
        data: &Arc<Mutex<DmabufFrame>>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_export_dmabuf_frame_v1::Event::Frame {
                width,
                height,
                format,
                mod_high,
                mod_low,
                num_objects,
                ..
            } => {
                if let Ok(mut data) = data.lock() {
                    if data.status != FRAME_PENDING {
                        warn!("[Wayland]: Frame event while frame is not pending!");
                        return;
                    }

                    data.fmt.w = width;
                    data.fmt.h = height;
                    data.fmt.format = format;
                    data.fmt.set_mod(mod_high, mod_low);
                    data.num_planes = num_objects as _;
                }
            }
            zwlr_export_dmabuf_frame_v1::Event::Object {
                index,
                fd,
                offset,
                stride,
                ..
            } => {
                if let Ok(mut data) = data.lock() {
                    if data.status != FRAME_PENDING {
                        warn!("[Wayland]: Object event while frame is not pending!");
                        return;
                    }

                    data.planes[index as usize] = FramePlane {
                        fd: fd.into_raw_fd(),
                        offset,
                        stride: stride as _,
                    }
                }
            }
            zwlr_export_dmabuf_frame_v1::Event::Ready { .. } => {
                if let Ok(mut data) = data.lock() {
                    if data.status != FRAME_PENDING {
                        warn!("[Wayland]: Ready event while frame is not pending!");
                        return;
                    }
                    data.status = FRAME_READY;
                }
                proxy.destroy();
            }
            zwlr_export_dmabuf_frame_v1::Event::Cancel { .. } => {
                if let Ok(mut data) = data.lock() {
                    warn!("[Wayland]: Frame capture failed.");
                    data.status = FRAME_FAILED;
                }
                proxy.destroy();
            }
            _ => {}
        }
    }
}

// Plumbing below

impl Dispatch<WlRegistry, ()> for WlClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: <WlRegistry as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZxdgOutputManagerV1, ()> for WlClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZxdgOutputManagerV1,
        _event: <ZxdgOutputManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrExportDmabufManagerV1, ()> for WlClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrExportDmabufManagerV1,
        _event: <ZwlrExportDmabufManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for WlClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: <WlRegistry as Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}
