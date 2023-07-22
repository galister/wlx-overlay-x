use libc::{ftruncate, shm_open, shm_unlink};
use libc::{O_CREAT, O_RDWR, S_IRUSR, S_IWUSR};
use std::os::fd::{FromRawFd, OwnedFd};
use std::{cell::RefCell, os::fd::AsRawFd, rc::Rc, sync::Arc};
use wayland_client::protocol::wl_buffer::WlBuffer;

use smithay_client_toolkit::reexports::{
    protocols::xdg::xdg_output::zv1::client::{
        zxdg_output_manager_v1::ZxdgOutputManagerV1,
        zxdg_output_v1::{self, ZxdgOutputV1},
    },
    protocols_wlr::{
        export_dmabuf::v1::client::{
            zwlr_export_dmabuf_frame_v1::{self, ZwlrExportDmabufFrameV1},
            zwlr_export_dmabuf_manager_v1::ZwlrExportDmabufManagerV1,
        },
        screencopy::v1::client::{
            zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
            zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
        },
    },
};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_output::{self, Transform, WlOutput},
        wl_registry::WlRegistry,
        wl_shm::WlShm,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};

use crate::gl::dmabuf::FRAME_FAILED;
use crate::gl::{
    dmabuf::{DmabufFrame, DmabufPlane, FRAME_PENDING, FRAME_READY},
    memfd::MemFdFrame,
};

pub struct OutputState {
    pub wl_output: WlOutput,
    pub id: u32,
    pub name: String,
    pub model: String,
    pub size: (i32, i32),
    pub logical_pos: (i32, i32),
    pub logical_size: (i32, i32),
    pub transform: WEnum<Transform>, // TODO: support upright displays
    done: bool,
}

pub struct WlClientState {
    pub connection: Arc<Connection>,
    pub xdg_output_mgr: ZxdgOutputManagerV1,
    pub maybe_shm: Option<WlShm>,
    pub maybe_wlr_dmabuf_mgr: Option<ZwlrExportDmabufManagerV1>,
    pub maybe_wlr_screencopy_mgr: Option<ZwlrScreencopyManagerV1>,
    pub outputs: Vec<OutputState>,
    pub desktop_rect: (i32, i32),
    pub queue: Rc<RefCell<EventQueue<Self>>>,
    pub queue_handle: QueueHandle<Self>,
    pub frame: DmabufFrame,
    pub memfd_frame: MemFdFrame,
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
            maybe_shm: globals.bind(&qh, 1..=1, ()).ok(),
            maybe_wlr_dmabuf_mgr: globals.bind(&qh, 1..=1, ()).ok(),
            maybe_wlr_screencopy_mgr: globals.bind(&qh, 1..=2, ()).ok(),
            outputs: vec![],
            desktop_rect: (0, 0),
            queue: Rc::new(RefCell::new(queue)),
            queue_handle: qh.clone(),
            frame: Default::default(),
            memfd_frame: Default::default(),
        };

        for o in globals.contents().clone_list().iter() {
            if o.interface == WlOutput::interface().name {
                let wl_output: WlOutput = globals.registry().bind(o.name, o.version, &qh, o.name);

                state.xdg_output_mgr.get_xdg_output(&wl_output, &qh, o.name);

                let output = OutputState {
                    wl_output,
                    id: o.name,
                    name: String::new(),
                    model: String::new(),
                    size: (0, 0),
                    logical_pos: (0, 0),
                    logical_size: (0, 0),
                    transform: WEnum::Unknown(0),
                    done: false,
                };

                state.outputs.push(output);
            }
        }

        state.dispatch();

        state
    }

    pub fn request_dmabuf_frame(&mut self, output_idx: usize) {
        if let Some(dmabuf_manager) = self.maybe_wlr_dmabuf_mgr.as_ref() {
            let frame = dmabuf_manager.capture_output(
                1,
                &self.outputs[output_idx].wl_output,
                &self.queue_handle,
                (),
            );

            self.dispatch();

            frame.destroy();
        }
    }

    pub fn request_screencopy_frame(&mut self, output_idx: usize) {
        if let Some(screencopy_manager) = self.maybe_wlr_screencopy_mgr.as_ref()
        {
            let frame = screencopy_manager.capture_output(
                1,
                &self.outputs[output_idx].wl_output,
                &self.queue_handle,
                (),
            );

            self.dispatch();

            frame.destroy();
        }
    }

    pub fn dispatch(&mut self) {
        let queue = self.queue.clone();
        let mut queue_mut = queue.borrow_mut();
        let _ = queue_mut.blocking_dispatch(self);
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
                    output.name = name;
                }
            }
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    output.logical_pos = (x, y);
                }
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    output.logical_size = (width, height);
                }
            }
            zxdg_output_v1::Event::Done => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
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
                    output.model = model;
                    output.transform = transform;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrExportDmabufFrameV1, ()> for WlClientState {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrExportDmabufFrameV1,
        event: <ZwlrExportDmabufFrameV1 as Proxy>::Event,
        _data: &(),
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
                if state.frame.status != FRAME_PENDING {
                    println!("[Wayland]: Frame event while frame is not pending!");
                    return;
                }

                println!("Format: {:X}", format);
                state.frame.format.width = width;
                state.frame.format.height = height;
                state.frame.format.format = format;
                state.frame.format.set_mod(mod_high, mod_low);
                state.frame.num_planes = num_objects as _;
            }
            zwlr_export_dmabuf_frame_v1::Event::Object {
                index,
                fd,
                offset,
                stride,
                ..
            } => {
                if state.frame.status != FRAME_PENDING {
                    println!("[Wayland]: Object event while frame is not pending!");
                    return;
                }

                println!("Object {} has fd {}", index, fd.as_raw_fd());

                state.frame.planes[index as usize] = DmabufPlane {
                    fd,
                    offset,
                    stride: stride as _,
                }
            }
            zwlr_export_dmabuf_frame_v1::Event::Ready { .. } => {
                if state.frame.status != FRAME_PENDING {
                    println!("[Wayland]: Ready event while frame is not pending!");
                    return;
                }
                state.frame.status = FRAME_READY;
            }
            zwlr_export_dmabuf_frame_v1::Event::Cancel { .. } => {
                println!("[Wayland]: Frame capture failed.");
                state.frame.status = FRAME_FAILED;
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for WlClientState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                if state.memfd_frame.status != FRAME_PENDING {
                    println!("[Wayland]: Buffer event while frame is not pending!");
                    return;
                }
                if let Ok(format) = format.into_result() {
                    state.memfd_frame.width = width;
                    state.memfd_frame.height = height;
                    state.memfd_frame.stride = stride;
                    state.memfd_frame.format = format;
                    state.memfd_frame.size = (stride * height) as _;
                }
            }
            zwlr_screencopy_frame_v1::Event::LinuxDmabuf { .. } => {}
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                if state.memfd_frame.status != FRAME_PENDING {
                    println!("[Wayland]: BufferDone event while frame is not pending!");
                    return;
                }
                let shm = state.maybe_shm.as_ref().unwrap();
                let f = &mut state.memfd_frame;

                unsafe {
                    let fd = shm_open(
                        f.shm_path.as_ptr() as _,
                        O_CREAT | O_RDWR,
                        S_IRUSR | S_IWUSR,
                    );
                    shm_unlink(f.shm_path.as_ptr() as _);
                    ftruncate(fd, f.size as _);

                    f.pool = shm.create_pool(fd, f.size as _, qhandle, ());
                    f.buffer = f.pool.create_buffer(
                        0,
                        f.width as _,
                        f.height as _,
                        f.stride as _,
                        f.format,
                        qhandle,
                        (),
                    );
                    f.fd = OwnedFd::from_raw_fd(fd);
                }
                proxy.copy(&state.memfd_frame.buffer);
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                if state.memfd_frame.status != FRAME_PENDING {
                    println!("[Wayland]: Ready event while frame is not pending!");
                    return;
                }
                state.memfd_frame.status = FRAME_READY;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                println!("[Wayland]: Frame capture failed.");
                state.memfd_frame.status = FRAME_FAILED;
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

impl Dispatch<ZwlrScreencopyManagerV1, ()> for WlClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as Proxy>::Event,
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

impl Dispatch<WlShm, ()> for WlClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlShm,
        _event: <WlShm as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShmPool, ()> for WlClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: <WlShmPool as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for WlClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        _event: <WlBuffer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}
