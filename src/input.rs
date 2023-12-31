use glam::Vec2;
use input_linux::{
    AbsoluteAxis, AbsoluteInfo, AbsoluteInfoSetup, EventKind, InputId, Key, RelativeAxis,
    UInputHandle,
};
use libc::{input_event, timeval};
use log::{error, info};
use once_cell::sync::Lazy;
use std::fs::File;
use std::{mem::transmute, sync::Mutex};
use strum::IntoEnumIterator;

use crate::keyboard::{VirtualKey, MODS_TO_KEYS};

pub static INPUT: Lazy<Mutex<Box<dyn InputProvider + Send>>> = Lazy::new(|| {
    if let Some(uinput) = UInputProvider::try_new() {
        info!("Initialized uinput.");
        return Mutex::new(Box::new(uinput));
    }
    error!("Could not create uinput provider. Keyboard/Mouse input will not work!");
    error!("Check if you're in `input` group: `id -nG`");
    Mutex::new(Box::new(DummyProvider {}))
});

pub trait InputProvider {
    fn mouse_move(&mut self, pos: Vec2);
    fn send_button(&self, button: u16, down: bool);
    fn wheel(&self, delta: i32);
    fn set_modifiers(&mut self, mods: u8);
    fn send_key(&self, key: u16, down: bool);
    fn set_desktop_extent(&mut self, extent: Vec2);
    fn on_new_frame(&mut self);
}

pub struct UInputProvider {
    handle: UInputHandle<File>,
    desktop_extent: Vec2,
    mouse_moved: bool,
    cur_modifiers: u8,
}

pub struct DummyProvider;

pub const MOUSE_LEFT: u16 = 0x110;
pub const MOUSE_RIGHT: u16 = 0x111;
pub const MOUSE_MIDDLE: u16 = 0x112;

const MOUSE_EXTENT: f32 = 32768.;

const EV_SYN: u16 = 0x0;
const EV_KEY: u16 = 0x1;
const EV_REL: u16 = 0x2;
const EV_ABS: u16 = 0x3;

impl UInputProvider {
    fn try_new() -> Option<Self> {
        if let Ok(file) = File::create("/dev/uinput") {
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
                        maximum: MOUSE_EXTENT as _,
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
                        maximum: MOUSE_EXTENT as _,
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

            for key in VirtualKey::iter() {
                let key: Key = unsafe { transmute(key as u16) };
                if handle.set_keybit(key).is_err() {
                    return None;
                }
            }

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
                    desktop_extent: Vec2::ZERO,
                    mouse_moved: false,
                    cur_modifiers: 0,
                });
            }
        }
        None
    }
}

impl InputProvider for UInputProvider {
    fn mouse_move(&mut self, pos: Vec2) {
        if self.mouse_moved {
            return;
        }
        self.mouse_moved = true;

        let pos = pos * (MOUSE_EXTENT / self.desktop_extent);

        let time = get_time();
        let events = [
            new_event(time, EV_ABS, AbsoluteAxis::X as _, pos.x as i32),
            new_event(time, EV_ABS, AbsoluteAxis::Y as _, pos.y as i32),
            new_event(time, EV_SYN, 0, 0),
        ];
        if let Err(res) = self.handle.write(&events) {
            error!("{}", res.to_string());
        }
    }
    fn send_button(&self, button: u16, down: bool) {
        let time = get_time();
        let events = [
            new_event(time, EV_KEY, button, down as _),
            new_event(time, EV_SYN, 0, 0),
        ];
        if let Err(res) = self.handle.write(&events) {
            error!("{}", res.to_string());
        }
    }
    fn wheel(&self, delta: i32) {
        let time = get_time();
        let events = [
            new_event(time, EV_REL, RelativeAxis::Wheel as _, delta),
            new_event(time, EV_SYN, 0, 0),
        ];
        if let Err(res) = self.handle.write(&events) {
            error!("{}", res.to_string());
        }
    }
    fn set_modifiers(&mut self, modifiers: u8) {
        let changed = self.cur_modifiers ^ modifiers;
        for i in 0..7 {
            let m = 1 << i;
            if changed & m != 0 {
                let vk = MODS_TO_KEYS.get(m).unwrap()[0] as u16;
                self.send_key(vk, modifiers & m != 0);
            }
        }
        self.cur_modifiers = modifiers;
    }
    fn send_key(&self, key: u16, down: bool) {
        let time = get_time();
        let events = [
            new_event(time, EV_KEY, key - 8, down as _),
            new_event(time, EV_SYN, 0, 0),
        ];
        if let Err(res) = self.handle.write(&events) {
            error!("{}", res.to_string());
        }
    }
    fn set_desktop_extent(&mut self, extent: Vec2) {
        info!("Desktop extent: {:?}", extent);
        self.desktop_extent = extent;
    }
    fn on_new_frame(&mut self) {
        self.mouse_moved = false;
    }
}

impl InputProvider for DummyProvider {
    fn mouse_move(&mut self, _pos: Vec2) {}
    fn send_button(&self, _button: u16, _down: bool) {}
    fn wheel(&self, _delta: i32) {}
    fn set_modifiers(&mut self, _modifiers: u8) {}
    fn send_key(&self, _key: u16, _down: bool) {}
    fn set_desktop_extent(&mut self, _extent: Vec2) {}
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
