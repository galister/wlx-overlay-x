use input_linux::{
    AbsoluteAxis, AbsoluteInfo, AbsoluteInfoSetup, EventKind, InputId, Key, RelativeAxis,
    UInputHandle,
};
use libc::{input_event, timeval};
use log::{error, info};
use once_cell::sync::Lazy;
use std::fs::File;
use std::{mem::transmute, sync::Mutex};

pub static INPUT: Lazy<Mutex<Box<dyn InputProvider + Send>>> = Lazy::new(|| {
    if let Some(uinput) = UInputProvider::try_new() {
        info!("Initialized uinput.");
        return Mutex::new(Box::new(uinput));
    }
    error!("Could not create uinput provider. Keyboard/Mouse input will not work!");
    error!("Check if you're in `input` group: `id -nG`");
    return Mutex::new(Box::new(DummyProvider {}));
});

pub trait InputProvider {
    fn mouse_move(&mut self, x: i32, y: i32);
    fn send_button(&self, button: u16, down: bool);
    fn wheel(&self, delta: i32);
    fn set_modifiers(&self);
    fn send_key(&self, key: u16, down: bool);
    fn set_desktop_extent(&mut self, extent: [i32; 2]);
    fn on_new_frame(&mut self);
}

pub struct UInputProvider {
    handle: UInputHandle<File>,
    desktop_extent: [i32; 2],
    mouse_moved: bool,
}

pub struct DummyProvider;

pub const MOUSE_LEFT: u16 = 0x110;
pub const MOUSE_RIGHT: u16 = 0x111;
pub const MOUSE_MIDDLE: u16 = 0x112;

const MOUSE_EXTENT: i32 = u16::MAX as _;

const EV_SYN: u16 = 0x0;
const EV_KEY: u16 = 0x1;
const EV_REL: u16 = 0x2;
const EV_ABS: u16 = 0x3;

impl UInputProvider {
    fn try_new() -> Option<Self> {
        if let Ok(file) = File::open("/dev/uinput") {
            let handle = UInputHandle::new(file);

            let id = InputId {
                bustype: 0x03,
                vendor: 0x4711,
                product: 0x0819,
                version: 5,
            };

            let name = b"WlxOverlay Keyboard-Mouse Hybrid Thing\0";

            let abs_info = vec![
                AbsoluteInfoSetup {
                    axis: input_linux::AbsoluteAxis::X,
                    info: AbsoluteInfo {
                        value: 0,
                        minimum: 0,
                        maximum: MOUSE_EXTENT,
                        fuzz: 0,
                        flat: 0,
                        resolution: 10,
                    },
                },
                AbsoluteInfoSetup {
                    axis: input_linux::AbsoluteAxis::Y,
                    info: AbsoluteInfo {
                        value: 0,
                        minimum: 0,
                        maximum: MOUSE_EXTENT,
                        fuzz: 0,
                        flat: 0,
                        resolution: 10,
                    },
                },
            ];

            if handle.set_evbit(EventKind::Key).is_err() {
                return None;
            }
            if handle.set_evbit(EventKind::Absolute).is_err() {
                return None;
            }
            if handle.set_evbit(EventKind::Relative).is_err() {
                return None;
            }

            for btn in MOUSE_LEFT..=MOUSE_MIDDLE {
                let key: Key = unsafe { transmute(btn) };
                if handle.set_keybit(key).is_err() {
                    return None;
                }
            }

            //TODO register keys

            if handle.set_absbit(AbsoluteAxis::X).is_err() {
                return None;
            }
            if handle.set_absbit(AbsoluteAxis::Y).is_err() {
                return None;
            }
            if handle.set_relbit(RelativeAxis::Wheel).is_err() {
                return None;
            }

            if handle.create(&id, name, 0, &abs_info).is_ok() {
                return Some(UInputProvider {
                    handle,
                    desktop_extent: [0, 0],
                    mouse_moved: false,
                });
            }
        }
        None
    }
}

impl InputProvider for UInputProvider {
    fn mouse_move(&mut self, x: i32, y: i32) {
        if self.mouse_moved {
            return;
        }
        self.mouse_moved = true;

        let mul_x = MOUSE_EXTENT / self.desktop_extent[0];
        let mul_y = MOUSE_EXTENT / self.desktop_extent[1];

        let time = get_time();
        let events = [
            new_event(time, EV_ABS, AbsoluteAxis::X as _, x * mul_x),
            new_event(time, EV_ABS, AbsoluteAxis::Y as _, y * mul_y),
            new_event(time, EV_SYN, 0, 0),
        ];
        let _ = self.handle.write(&events);
    }
    fn send_button(&self, button: u16, down: bool) {
        let time = get_time();
        let events = [
            new_event(time, EV_KEY, button, down as _),
            new_event(time, EV_SYN, 0, 0),
        ];
        let _ = self.handle.write(&events);
    }
    fn wheel(&self, delta: i32) {
        let time = get_time();
        let events = [
            new_event(time, EV_REL, RelativeAxis::Wheel as _, delta),
            new_event(time, EV_SYN, 0, 0),
        ];
        let _ = self.handle.write(&events);
    }
    fn set_modifiers(&self) {}
    fn send_key(&self, key: u16, down: bool) {
        let time = get_time();
        let events = [
            new_event(time, EV_KEY, key - 8, down as _),
            new_event(time, EV_SYN, 0, 0),
        ];
        let _ = self.handle.write(&events);
    }
    fn set_desktop_extent(&mut self, extent: [i32; 2]) {
        info!("Desktop extent: {}x{}", extent[0], extent[1]);
        self.desktop_extent = extent;
    }
    fn on_new_frame(&mut self) {
        self.mouse_moved = false;
    }
}

impl InputProvider for DummyProvider {
    fn mouse_move(&mut self, _x: i32, _y: i32) {}
    fn send_button(&self, _button: u16, _down: bool) {}
    fn wheel(&self, _delta: i32) {}
    fn set_modifiers(&self) {}
    fn send_key(&self, _key: u16, _down: bool) {}
    fn set_desktop_extent(&mut self, _extent: [i32; 2]) {}
    fn on_new_frame(&mut self) {}
}

#[inline]
fn get_time() -> timeval {
    let mut time = timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    unsafe { libc::gettimeofday(&mut time, std::ptr::null_mut()) };
    time
}

#[inline]
fn new_event(time: timeval, type_: u16, code: u16, value: i32) -> input_event {
    input_event {
        time,
        type_,
        code,
        value,
    }
}
