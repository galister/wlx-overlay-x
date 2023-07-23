use std::io::Cursor;
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::cell::RefCell;

use crate::desktop::frame::{DrmFormat, FrameFormat, FramePlane};

use ashpd::{
    desktop::screencast::{CursorMode, PersistMode, Screencast, SourceType},
    WindowIdentifier,
};

use libspa_sys::{spa_pod, spa_video_info_raw};
use pipewire::prelude::*;
use pipewire::properties;
use pipewire::spa::pod::serialize::PodSerializer;
use pipewire::spa::pod::{ChoiceValue, Object, Property, PropertyFlags, Value};
use pipewire::spa::utils::{Choice, ChoiceFlags, Fraction, Rectangle};
use pipewire::spa::utils::{ChoiceEnum, Id};
use pipewire::stream::{Stream, StreamFlags};
use pipewire::{Context, Error, MainLoop};

pub struct PipewireCapture {}

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

pub fn pipewire_init_stream<F>(
    name: &str,
    node_id: u32,
    fps: u32,
    formats: Vec<DrmFormat>,
    on_frame: F,
) -> Result<(), Error>
where
    F: Fn(&FrameFormat, &Vec<FramePlane>) + 'static,
{
    let main_loop = MainLoop::new()?;
    let context = Context::new(&main_loop)?;
    let _core = context.connect(None)?;

    let stream: Rc<RefCell<Option<Stream<i32>>>> = Rc::new(RefCell::new(None));
    let stream_clone = stream.clone();

    let format: Rc<RefCell<Option<FrameFormat>>> = Rc::new(RefCell::new(None));
    let format_clone = format.clone();

    let stream_inner = Stream::<i32>::with_user_data(
        &main_loop,
        name,
        properties! {
            *pipewire::keys::MEDIA_TYPE => "Video",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Screen",
        },
        0,
    )
    .param_changed(move |_, id, param| {
        if param.is_null() || *id != libspa_sys::SPA_PARAM_Format as _ {
            return;
        }
        let mut maybe_info = MaybeUninit::<spa_video_info_raw>::zeroed();

        unsafe {
            libspa_sys::spa_format_video_raw_parse(param, maybe_info.as_mut_ptr());
        }

        let info = unsafe { maybe_info.assume_init() };

        let format = FrameFormat {
            w: info.size.width,
            h: info.size.height,
            format: info.format,
            modifier: info.modifier,
            size: 0, // TODO verify
        };
        format_clone.replace(Some(format));

        let params = format_dmabuf_params();

        if let Some(ref stream) = *stream_clone.borrow() {
            let _ = stream.update_params(&mut [params.as_ptr() as _]);
        }
    })
    .state_changed(|old, new| {
        println!("Stream state changed: {:?} -> {:?}", old, new);
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
                return;
            }
            let planes: Vec<FramePlane> = datas
                .iter()
                .map(|p| FramePlane {
                    fd: p.as_raw().fd as _,
                    offset: p.chunk().offset(),
                    stride: p.chunk().stride(),
                })
                .collect();

            if let Some(ref format) = *format.borrow() {
                on_frame(format, &planes);
            }
        }
    })
    .create()?;

    let mut format_params: Vec<*const spa_pod> = formats
        .iter()
        .filter_map(|f| {
            let spa_video_format = fourcc_to_spa_video_format(f.code)?;
            Some(format_get_params(spa_video_format, f.modifier, fps).as_ptr() as _)
        })
        .collect();

    stream.replace(Some(stream_inner));

    if let Some(ref stream_inner) = *stream.borrow() {
        stream_inner.connect(
            pipewire::spa::Direction::Input,
            Some(node_id),
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            format_params.as_mut_slice(),
        )?;
    }

    main_loop.run();
    unsafe { pipewire::deinit() };

    Ok(())
}

fn fourcc_to_spa_video_format(fourcc: u32) -> Option<u32> {
    match fourcc {
        //DRM_FORMAT_ARGB8888 (order on fourcc are reversed ARGB = BGRA)
        0x34325241 => Some(libspa_sys::SPA_VIDEO_FORMAT_BGRA),
        //DRM_FORMAT_ABGR8888
        0x34324241 => Some(libspa_sys::SPA_VIDEO_FORMAT_RGBA),
        //DRM_FORMAT_XRGB8888
        0x34325258 => Some(libspa_sys::SPA_VIDEO_FORMAT_BGRx),
        //DRM_FORMAT_XBGR8888
        0x34324258 => Some(libspa_sys::SPA_VIDEO_FORMAT_RGBx),
        _ => None,
    }
}

fn format_dmabuf_params() -> Vec<u8> {
    let pod = Value::Object(Object {
        type_: libspa_sys::SPA_TYPE_OBJECT_ParamBuffers,
        id: libspa_sys::SPA_PARAM_Buffers,
        properties: vec![Property {
            key: libspa_sys::SPA_PARAM_BUFFERS_dataType,
            flags: PropertyFlags::empty(),
            value: Value::Id(Id(libspa_sys::SPA_DATA_DmaBuf)),
        }],
    });
    let (c, _) = PodSerializer::serialize(Cursor::new(Vec::new()), &pod).unwrap();
    c.into_inner()
}

fn format_get_params(format: u32, modifier: u64, fps: u32) -> Vec<u8> {
    let pod = Value::Object(Object {
        type_: libspa_sys::SPA_TYPE_OBJECT_Format,
        id: libspa_sys::SPA_PARAM_EnumFormat,
        properties: vec![
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
                key: libspa_sys::SPA_FORMAT_VIDEO_format,
                flags: PropertyFlags::empty(),
                value: Value::Id(Id(format)),
            },
            Property {
                key: libspa_sys::SPA_FORMAT_VIDEO_modifier,
                flags: PropertyFlags::MANDATORY | PropertyFlags::DONT_FIXATE,
                value: Value::Id(Id(modifier as _)),
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
        ],
    });

    let (c, _) = PodSerializer::serialize(Cursor::new(Vec::new()), &pod).unwrap();
    c.into_inner()
}
