use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};
use std::thread::JoinHandle;

use crate::desktop::frame::{
    texture_load_dmabuf, texture_load_memfd, texture_load_memptr, MemPtrFrame,
};
use crate::overlay::OverlayRenderer;
use crate::{
    desktop::frame::{DmabufFrame, DrmFormat, FrameFormat, FramePlane, MemFdFrame},
    gl::egl::{
        eglQueryDmaBufFormatsEXT, eglQueryDmaBufModifiersEXT, DRM_FORMAT_ABGR8888,
        DRM_FORMAT_ARGB8888, DRM_FORMAT_XBGR8888, DRM_FORMAT_XRGB8888, EGL_TRUE,
    },
};

use ashpd::{
    desktop::screencast::{CursorMode, PersistMode, Screencast, SourceType},
    WindowIdentifier,
};

use libspa_sys::{
    spa_pod, spa_video_info_raw, SPA_DATA_DmaBuf, SPA_DATA_MemFd, SPA_DATA_MemPtr,
    SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_RGBx, SPA_VIDEO_FORMAT_BGRA, SPA_VIDEO_FORMAT_RGBA,
};
use log::{error, info, warn};
use once_cell::sync::Lazy;
use pipewire::prelude::*;
use pipewire::properties;
use pipewire::spa::data::DataType;
use pipewire::spa::pod::serialize::PodSerializer;
use pipewire::spa::pod::{ChoiceValue, Object, Property, PropertyFlags, Value};
use pipewire::spa::utils::{Choice, ChoiceEnum, ChoiceFlags, Fraction, Id, Rectangle};
use pipewire::stream::{Stream, StreamFlags};
use pipewire::{Context, Error, MainLoop};
use stereokit::StereoKitMultiThread;

static FORMATS: Lazy<Arc<Vec<DrmFormat>>> = Lazy::new(|| Arc::new(load_dmabuf_formats()));

pub async fn pipewire_select_screen(token: Option<&str>) -> Result<u32, ashpd::Error> {
    let proxy = Screencast::new().await?;
    let session = proxy.create_session().await?;

    proxy
        .select_sources(
            &session,
            CursorMode::Embedded,
            SourceType::Monitor | SourceType::Window,
            false,
            token,
            PersistMode::ExplicitlyRevoked,
        )
        .await?;

    let response = proxy
        .start(&session, &WindowIdentifier::default())
        .await?
        .response()?;

    if let Some(stream) = response.streams().first() {
        return Ok(stream.pipe_wire_node_id());
    }

    return Err(ashpd::Error::NoResponse);
}

pub enum PipewireFrame {
    Dmabuf(DmabufFrame),
    MemFd(MemFdFrame),
    MemPtr(MemPtrFrame),
}

struct StreamData {
    format: Option<FrameFormat>,
    stream: Option<Stream<i32>>,
}

impl StreamData {
    fn new() -> Self {
        StreamData {
            format: None,
            stream: None,
        }
    }
}

pub struct PipewireCapture {
    name: Arc<String>,
    node_id: u32,
    fps: u32,
    dmabuf: bool,
    frame: Arc<Mutex<Option<PipewireFrame>>>,
    handle: Option<JoinHandle<Result<(), Error>>>,
}

impl OverlayRenderer for PipewireCapture {
    fn init(&mut self, _sk: &stereokit::SkDraw) {
        self.start();
    }
    fn pause(&mut self, _app: &mut crate::AppState) {}
    fn resume(&mut self, _app: &mut crate::AppState) {}
    fn render(&mut self, sk: &stereokit::SkDraw, tex: &stereokit::Tex, _app: &mut crate::AppState) {
        if let Ok(mut pw_frame) = self.frame.lock() {
            if let Some(pw_frame) = pw_frame.take() {
                match pw_frame {
                    PipewireFrame::Dmabuf(frame) => {
                        if frame.is_valid() {
                            let handle =
                                unsafe { sk.tex_get_surface(&tex.as_ref()) as usize as u32 };
                            texture_load_dmabuf(handle, &frame);
                        }
                    }
                    PipewireFrame::MemFd(frame) => {
                        let handle = unsafe { sk.tex_get_surface(&tex.as_ref()) as usize as u32 };
                        texture_load_memfd(handle, &frame);
                    }
                    PipewireFrame::MemPtr(frame) => {
                        let handle = unsafe { sk.tex_get_surface(&tex.as_ref()) as usize as u32 };
                        texture_load_memptr(handle, &frame);
                    }
                }
            }
        }
    }
}

impl PipewireCapture {
    pub fn new(name: String, node_id: u32, fps: u32, dmabuf: bool) -> Self {
        PipewireCapture {
            name: Arc::new(name),
            node_id,
            fps,
            dmabuf,
            frame: Arc::new(Mutex::new(None)),
            handle: None,
        }
    }

    fn start(&mut self) {
        self.handle = Some(main_loop(
            self.name.clone(),
            self.node_id,
            self.fps,
            self.dmabuf,
            self.frame.clone(),
        ));
    }
}

fn main_loop(
    name: Arc<String>,
    node_id: u32,
    fps: u32,
    dmabuf: bool,
    frame: Arc<Mutex<Option<PipewireFrame>>>,
) -> JoinHandle<Result<(), Error>> {
    std::thread::spawn(move || {
        let main_loop = MainLoop::new()?;
        let context = Context::new(&main_loop)?;
        let _core = context.connect(None)?;

        let data = Arc::new(RwLock::new(StreamData::new()));
        let data_copy = data.clone();
        let data_copy2 = data.clone();

        let name_copy = name.clone();
        let name_copy2 = name.clone();
        let name_copy3 = name.clone();

        let stream = Stream::<i32>::with_user_data(
            &main_loop,
            &name,
            properties! {
                *pipewire::keys::MEDIA_TYPE => "Video",
                *pipewire::keys::MEDIA_CATEGORY => "Capture",
                *pipewire::keys::MEDIA_ROLE => "Screen",
            },
            0,
        )
        .param_changed(move |id, _, param| {
            if param.is_null() || id != libspa_sys::SPA_PARAM_Format {
                return;
            }
            let mut maybe_info = MaybeUninit::<spa_video_info_raw>::zeroed();
            unsafe {
                if libspa_sys::spa_format_video_raw_parse(param, maybe_info.as_mut_ptr()) < 0 {
                    return;
                }
            }
            let info = unsafe { maybe_info.assume_init() };

            let format = FrameFormat {
                w: info.size.width,
                h: info.size.height,
                format: spa_to_fourcc(info.format),
                modifier: info.modifier,
            };

            info!("{}: {:?}", &name_copy, format);

            if let Some(ref mut data) = data_copy.write().ok() {
                data.format = Some(format);

                if let Some(stream) = &data.stream {
                    let params = format_dmabuf_params();
                    if let Err(e) = stream.update_params(&mut [params.as_ptr() as _]) {
                        error!("{}: failed to update params: {}", &name_copy, e);
                    }
                }
            }
        })
        .state_changed(move |old, new| {
            info!(
                "{}: stream state changed: {:?} -> {:?}",
                &name_copy2, old, new
            );
        })
        .process(move |stream, _| {
            let mut maybe_buffer = None;
            // discard all but the freshest ingredients
            while let Some(buffer) = stream.dequeue_buffer() {
                maybe_buffer = Some(buffer);
            }

            if let Some(mut buffer) = maybe_buffer {
                let datas = buffer.datas_mut();
                if datas.len() < 1 {
                    info!("{}: no data", &name_copy3);
                    return;
                }

                if let Ok(Some(format)) = data_copy2.read().and_then(|d| Ok(d.format)) {
                    let planes: Vec<FramePlane> = datas
                        .iter()
                        .map(|p| FramePlane {
                            fd: p.as_raw().fd as _,
                            offset: p.chunk().offset(),
                            stride: p.chunk().stride(),
                        })
                        .collect();

                    if let Ok(mut frame) = frame.lock() {
                        match datas[0].type_() {
                            DataType::DmaBuf => {
                                let mut dmabuf = DmabufFrame::default();
                                dmabuf.fmt = format;
                                dmabuf.num_planes = planes.len();
                                for i in 0..planes.len() {
                                    dmabuf.planes[i] = planes[i];
                                }

                                *frame = Some(PipewireFrame::Dmabuf(dmabuf));
                            }
                            DataType::MemFd => {
                                *frame = Some(PipewireFrame::MemFd(MemFdFrame {
                                    fmt: format,
                                    plane: FramePlane {
                                        fd: datas[0].as_raw().fd as _,
                                        offset: datas[0].chunk().offset(),
                                        stride: datas[0].chunk().stride(),
                                    },
                                }));
                            }
                            DataType::MemPtr => {
                                *frame = Some(PipewireFrame::MemPtr(MemPtrFrame {
                                    fmt: format,
                                    ptr: datas[0].as_raw().data as _,
                                }));
                            }
                            _ => panic!("Unknown data type"),
                        }
                    }
                } else {
                    info!("{}: no format", &name_copy3);
                }
            }
        })
        .create()?;

        let mut format_params: Vec<SpaPod> = if dmabuf {
            FORMATS
                .iter()
                .map(|f| format_get_params(Some(f), fps))
                .collect()
        } else {
            Vec::with_capacity(0)
        };
        format_params.push(format_get_params(None, fps));

        let mut format_ptrs = format_params
            .iter()
            .map(|f| f.as_ptr() as _)
            .collect::<Vec<_>>();

        stream.connect(
            pipewire::spa::Direction::Input,
            Some(node_id),
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            format_ptrs.as_mut_slice(),
        )?;

        if let Ok(mut data) = data.write() {
            data.stream = Some(stream);
        }

        main_loop.run();
        warn!("{}: pipewire loop exited", &name);
        Ok::<(), Error>(())
    })
}

fn load_dmabuf_formats() -> Vec<DrmFormat> {
    let mut num_fmt = 0;
    let mut out_fmts = Vec::new();

    if eglQueryDmaBufFormatsEXT(0, null_mut(), &mut num_fmt) != EGL_TRUE {
        return out_fmts;
    }

    let mut fmts = Vec::with_capacity(num_fmt as usize);
    unsafe { fmts.set_len(num_fmt as _) };
    if eglQueryDmaBufFormatsEXT(num_fmt, fmts.as_mut_ptr(), &mut num_fmt) != EGL_TRUE {
        return out_fmts;
    }

    let wanted_fmts = [
        DRM_FORMAT_ARGB8888,
        DRM_FORMAT_ABGR8888,
        DRM_FORMAT_XRGB8888,
        DRM_FORMAT_XBGR8888,
    ];

    let valid_fmts = fmts
        .iter()
        .filter(|f| wanted_fmts.contains(f))
        .copied()
        .collect::<Vec<_>>();

    for f in valid_fmts {
        let mut num_mod = 0;
        if eglQueryDmaBufModifiersEXT(f, 0, null_mut(), 0, &mut num_mod) != EGL_TRUE {
            continue;
        }

        let mut mods = Vec::with_capacity(num_mod as usize);
        unsafe { mods.set_len(num_mod as _) };
        if eglQueryDmaBufModifiersEXT(f, num_mod, mods.as_mut_ptr(), 0, &mut num_mod) != EGL_TRUE {
            continue;
        }

        out_fmts.push(DrmFormat {
            code: f,
            modifiers: mods,
        });
    }

    out_fmts
}

struct SpaPod {
    data: Vec<u8>,
}

impl SpaPod {
    fn as_ptr(&self) -> *const spa_pod {
        self.data.as_ptr() as _
    }
}

fn format_dmabuf_params() -> SpaPod {
    let data_types = (1 << SPA_DATA_MemFd) | (1 << SPA_DATA_MemPtr) | (1 << SPA_DATA_DmaBuf);

    let pod = Value::Object(Object {
        type_: libspa_sys::SPA_TYPE_OBJECT_ParamBuffers,
        id: libspa_sys::SPA_PARAM_Buffers,
        properties: vec![Property {
            key: libspa_sys::SPA_PARAM_BUFFERS_dataType,
            flags: PropertyFlags::empty(),
            value: Value::Int(data_types),
        }],
    });
    let (c, _) = PodSerializer::serialize(Cursor::new(Vec::new()), &pod).unwrap();
    SpaPod {
        data: c.into_inner(),
    }
}

fn format_get_params(fmt: Option<&DrmFormat>, fps: u32) -> SpaPod {
    let mut properties = vec![
        Property {
            key: libspa_sys::SPA_FORMAT_mediaType,
            flags: PropertyFlags::empty(),
            value: Value::Id(Id(libspa_sys::SPA_MEDIA_TYPE_video)),
        },
        Property {
            key: libspa_sys::SPA_FORMAT_mediaSubtype,
            flags: PropertyFlags::empty(),
            value: Value::Id(Id(libspa_sys::SPA_MEDIA_SUBTYPE_raw)),
        },
        Property {
            key: libspa_sys::SPA_FORMAT_VIDEO_size,
            flags: PropertyFlags::empty(),
            value: Value::Choice(ChoiceValue::Rectangle(Choice(
                ChoiceFlags::from_bits_truncate(0),
                ChoiceEnum::Range {
                    default: Rectangle {
                        width: 256,
                        height: 256,
                    },
                    min: Rectangle {
                        width: 1,
                        height: 1,
                    },
                    max: Rectangle {
                        width: 8192,
                        height: 8192,
                    },
                },
            ))),
        },
        Property {
            key: libspa_sys::SPA_FORMAT_VIDEO_framerate,
            flags: PropertyFlags::empty(),
            value: Value::Choice(ChoiceValue::Fraction(Choice(
                ChoiceFlags::from_bits_truncate(0),
                ChoiceEnum::Range {
                    default: Fraction { num: fps, denom: 1 },
                    min: Fraction { num: 0, denom: 1 },
                    max: Fraction {
                        num: 1000,
                        denom: 1,
                    },
                },
            ))),
        },
    ];
    if let Some(fmt) = fmt {
        properties.push(Property {
            key: libspa_sys::SPA_FORMAT_VIDEO_format,
            flags: PropertyFlags::empty(),
            value: Value::Id(Id(fourcc_to_spa(fmt.code))),
        });

        if fmt.modifiers.len() > 0 {
            properties.push(Property {
                key: libspa_sys::SPA_FORMAT_VIDEO_modifier,
                flags: PropertyFlags::MANDATORY | PropertyFlags::DONT_FIXATE,
                value: Value::Choice(ChoiceValue::Long(Choice(
                    ChoiceFlags::from_bits_truncate(0), 
                    ChoiceEnum::Enum{
                        default: fmt.modifiers[0] as _,
                        alternatives: fmt.modifiers.iter().skip(1).map(|m| { *m as _ }).collect()
                    }))),
            });
        }
    } else {
        properties.push(Property {
            key: libspa_sys::SPA_FORMAT_VIDEO_format,
            flags: PropertyFlags::empty(),
            value: Value::Choice(ChoiceValue::Id(Choice(
                ChoiceFlags::from_bits_truncate(0),
                ChoiceEnum::Enum {
                    default: Id(SPA_VIDEO_FORMAT_RGBA),
                    alternatives: vec![
                        Id(SPA_VIDEO_FORMAT_BGRA),
                        Id(SPA_VIDEO_FORMAT_RGBx),
                        Id(SPA_VIDEO_FORMAT_BGRx),
                    ],
                },
            ))),
        });
    }

    let pod = Value::Object(Object {
        type_: libspa_sys::SPA_TYPE_OBJECT_Format,
        id: libspa_sys::SPA_PARAM_EnumFormat,
        properties,
    });

    let (c, _) = PodSerializer::serialize(Cursor::new(Vec::new()), &pod).unwrap();
    SpaPod {
        data: c.into_inner(),
    }
}

fn fourcc_to_spa(fourcc: u32) -> u32 {
    match fourcc {
        DRM_FORMAT_ARGB8888 => SPA_VIDEO_FORMAT_BGRA,
        DRM_FORMAT_ABGR8888 => SPA_VIDEO_FORMAT_RGBA,
        DRM_FORMAT_XRGB8888 => SPA_VIDEO_FORMAT_BGRx,
        DRM_FORMAT_XBGR8888 => SPA_VIDEO_FORMAT_RGBx,
        _ => panic!("Unsupported format"),
    }
}

#[allow(non_upper_case_globals)]
fn spa_to_fourcc(spa: u32) -> u32 {
    match spa {
        SPA_VIDEO_FORMAT_BGRA => DRM_FORMAT_ARGB8888,
        SPA_VIDEO_FORMAT_RGBA => DRM_FORMAT_ABGR8888,
        SPA_VIDEO_FORMAT_BGRx => DRM_FORMAT_XRGB8888,
        SPA_VIDEO_FORMAT_RGBx => DRM_FORMAT_XBGR8888,
        _ => panic!("Unsupported format"),
    }
}

