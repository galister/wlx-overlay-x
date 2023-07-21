use smithay_client_toolkit::reexports::{
    protocols::xdg::xdg_output::zv1::client::{
        zxdg_output_manager_v1::ZxdgOutputManagerV1,
        zxdg_output_v1::{self, ZxdgOutputV1},
    },
    protocols_wlr::export_dmabuf::v1::client::zwlr_export_dmabuf_manager_v1::ZwlrExportDmabufManagerV1,
};
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_output::{Transform, WlOutput},
        wl_registry::WlRegistry,
    },
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
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
    pub connection: Connection,
    pub xdg_output_mgr: ZxdgOutputManagerV1,
    pub maybe_wlr_dmabuf_mgr: Option<ZwlrExportDmabufManagerV1>,
    pub outputs: Vec<OutputState>,
    pub desktop_rect: (i32, i32),
}

impl WlClientState {
    pub fn new() -> Self {
        let connection = Connection::connect_to_env().expect("wayland connection");
        let (globals, mut queue) =
            registry_queue_init::<Self>(&connection).expect("wayland globals");
        let qh = queue.handle();

        let mut state = Self {
            connection,
            xdg_output_mgr: globals
                .bind(&qh, 2..=3, ())
                .expect(ZxdgOutputManagerV1::interface().name),
            maybe_wlr_dmabuf_mgr: globals.bind(&qh, 1..=1, ()).ok(),
            outputs: vec![],
            desktop_rect: (0, 0),
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

        queue.blocking_dispatch(&mut state).expect("dispatch");

        state
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
            wayland_client::protocol::wl_output::Event::Mode { width, height, .. } => {
                if let Some(output) = state.outputs.iter_mut().find(|o| o.id == *data) {
                    output.size = (width, height);
                }
            }
            wayland_client::protocol::wl_output::Event::Geometry {
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
