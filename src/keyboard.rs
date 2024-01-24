use std::{
    collections::HashMap,
    io::Cursor,
    process::{Child, Command},
    str::FromStr,
    sync::Arc,
};

use crate::{
    config,
    gui::{color_parse, Canvas, Control},
    input::INPUT,
    overlay::OverlayData,
    AppSession,
};
use glam::{vec2, vec3};
use idmap::{idmap, IdMap};
use idmap_derive::IntegerId;
use log::error;
use once_cell::sync::Lazy;
use regex::Regex;
use rodio::{Decoder, OutputStream, Source};
use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString};

const PIXELS_PER_UNIT: f32 = 80.;
const BUTTON_PADDING: f32 = 4.;

pub fn create_keyboard(session: &AppSession) -> OverlayData {
    let size = vec2(
        LAYOUT.row_size * PIXELS_PER_UNIT,
        (LAYOUT.main_layout.len() as f32) * PIXELS_PER_UNIT,
    );

    let data = KeyboardData {
        modifiers: 0,
        processes: vec![],
        audio_stream: None,
    };

    let mut canvas = Canvas::new(size.x as _, size.y as _, data);

    canvas.bg_color = color_parse("#101010");
    canvas.panel(0., 0., size.x, size.y);

    canvas.font_size = 18;
    canvas.bg_color = color_parse("#202020");

    let unit_size = size.x / LAYOUT.row_size;
    let h = unit_size - 2. * BUTTON_PADDING;

    for row in 0..LAYOUT.key_sizes.len() {
        let y = unit_size * (row as f32) + BUTTON_PADDING;
        let mut sum_size = 0f32;

        for col in 0..LAYOUT.key_sizes[row].len() {
            let my_size = LAYOUT.key_sizes[row][col];
            let x = unit_size * sum_size + BUTTON_PADDING;
            let w = unit_size * my_size - 2. * BUTTON_PADDING;

            if let Some(key) = LAYOUT.main_layout[row][col].as_ref() {
                let mut maybe_state: Option<KeyButtonData> = None;
                if let Ok(vk) = VirtualKey::from_str(key) {
                    if let Some(mods) = KEYS_TO_MODS.get(vk) {
                        maybe_state = Some(KeyButtonData::Modifier {
                            modifier: *mods,
                            sticky: false,
                            pressed: false,
                        });
                    } else {
                        maybe_state = Some(KeyButtonData::Key { vk, pressed: false });
                    }
                } else if let Some(macro_verbs) = LAYOUT.macros.get(key) {
                    maybe_state = Some(KeyButtonData::Macro {
                        verbs: key_events_for_macro(macro_verbs),
                    });
                } else if let Some(exec_args) = LAYOUT.exec_commands.get(key) {
                    maybe_state = Some(KeyButtonData::Exec {
                        program: exec_args.first().unwrap().clone(),
                        args: exec_args.iter().skip(1).cloned().collect(),
                    });
                } else {
                    error!("Unknown key: {}", key);
                }

                if let Some(state) = maybe_state {
                    let label = LAYOUT.label_for_key(key);
                    let idx = canvas.key_button(x, y, w, h, &label);
                    let button = &mut canvas.controls[idx];
                    button.state = Some(state);
                    button.on_press = Some(key_press);
                    button.on_release = Some(key_release);
                    button.test_highlight = Some(test_highlight);
                }
            }

            sum_size += my_size;
        }
    }

    OverlayData {
        name: Arc::from("Kbd"),
        show_hide: true,
        width: LAYOUT.row_size * 0.05 * session.config.keyboard_scale,
        size: (canvas.width as _, canvas.height as _),
        grabbable: true,
        spawn_point: vec3(0., -0.5, -1.),
        backend: Box::new(canvas),
        ..Default::default()
    }
}

fn key_press(
    control: &mut Control<KeyboardData, KeyButtonData>,
    session: &AppSession,
    data: &mut KeyboardData,
) {
    match control.state.as_mut() {
        Some(KeyButtonData::Key { vk, pressed }) => {
            if let Ok(input) = INPUT.lock() {
                data.key_click(session);
                input.send_key(*vk as _, true);
                *pressed = true;
            }
        }
        Some(KeyButtonData::Modifier {
            modifier,
            sticky,
            pressed,
        }) => {
            *sticky = data.modifiers & *modifier == 0;
            data.modifiers |= *modifier;
            if let Ok(mut input) = INPUT.lock() {
                data.key_click(session);
                input.set_modifiers(data.modifiers);
                *pressed = true;
            }
        }
        Some(KeyButtonData::Macro { verbs }) => {
            if let Ok(input) = INPUT.lock() {
                data.key_click(session);
                for (vk, press) in verbs {
                    input.send_key(*vk as _, *press);
                }
            }
        }
        Some(KeyButtonData::Exec { program, args }) => {
            // Reap previous processes
            data.processes
                .retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));

            data.key_click(session);
            if let Ok(child) = Command::new(program).args(args).spawn() {
                data.processes.push(child);
            }
        }
        None => {}
    }
}

fn key_release(control: &mut Control<KeyboardData, KeyButtonData>, data: &mut KeyboardData) {
    match control.state.as_mut() {
        Some(KeyButtonData::Key { vk, pressed }) => {
            if let Ok(input) = INPUT.lock() {
                input.send_key(*vk as _, false);
            }
            *pressed = false;
        }
        Some(KeyButtonData::Modifier {
            modifier,
            sticky,
            pressed,
        }) => {
            if !*sticky {
                data.modifiers &= !*modifier;
                if let Ok(mut input) = INPUT.lock() {
                    input.set_modifiers(data.modifiers);
                }
                *pressed = false;
            }
        }
        _ => {}
    }
}

fn test_highlight(
    control: &mut Control<KeyboardData, KeyButtonData>,
    _data: &mut KeyboardData,
) -> bool {
    match control.state.as_ref() {
        Some(KeyButtonData::Key { pressed, .. }) => *pressed,
        Some(KeyButtonData::Modifier { pressed, .. }) => *pressed,
        _ => false,
    }
}

struct KeyboardData {
    modifiers: KeyModifier,
    processes: Vec<Child>,
    audio_stream: Option<OutputStream>,
}

impl KeyboardData {
    fn key_click(&mut self, session: &AppSession) {
        if !session.config.keyboard_sound_enabled {
            return;
        }
        let wav = include_bytes!("res/421581.wav");
        let cursor = Cursor::new(wav);
        let source = Decoder::new_wav(cursor).unwrap();
        self.audio_stream = None;
        if let Ok((stream, handle)) = OutputStream::try_default() {
            let _ = handle.play_raw(source.convert_samples());
            self.audio_stream = Some(stream);
        } else {
            error!("Failed to play key click");
        }
    }
}

enum KeyButtonData {
    Key {
        vk: VirtualKey,
        pressed: bool,
    },
    Modifier {
        modifier: KeyModifier,
        sticky: bool,
        pressed: bool,
    },
    Macro {
        verbs: Vec<(VirtualKey, bool)>,
    },
    Exec {
        program: String,
        args: Vec<String>,
    },
}

static KEYS_TO_MODS: Lazy<IdMap<VirtualKey, KeyModifier>> = Lazy::new(|| {
    idmap! {
        VirtualKey::LShift => SHIFT,
        VirtualKey::RShift => SHIFT,
        VirtualKey::Caps => CAPS_LOCK,
        VirtualKey::LCtrl => CTRL,
        VirtualKey::RCtrl => CTRL,
        VirtualKey::LAlt => ALT,
        VirtualKey::NumLock => NUM_LOCK,
        VirtualKey::LSuper => SUPER,
        VirtualKey::RSuper => SUPER,
        VirtualKey::Meta => META,
    }
});

pub static MODS_TO_KEYS: Lazy<IdMap<KeyModifier, Vec<VirtualKey>>> = Lazy::new(|| {
    idmap! {
        SHIFT => vec![VirtualKey::LShift, VirtualKey::RShift],
        CAPS_LOCK => vec![VirtualKey::Caps],
        CTRL => vec![VirtualKey::LCtrl, VirtualKey::RCtrl],
        ALT => vec![VirtualKey::LAlt],
        NUM_LOCK => vec![VirtualKey::NumLock],
        SUPER => vec![VirtualKey::LSuper, VirtualKey::RSuper],
        META => vec![VirtualKey::Meta],
    }
});

static LAYOUT: Lazy<Layout> = Lazy::new(Layout::load_from_disk);

static MACRO_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Za-z0-1_-]+)(?: +(UP|DOWN))?$").unwrap());

#[derive(Debug, Deserialize, Serialize)]
pub struct Layout {
    name: String,
    row_size: f32,
    key_sizes: Vec<Vec<f32>>,
    main_layout: Vec<Vec<Option<String>>>,
    exec_commands: HashMap<String, Vec<String>>,
    macros: HashMap<String, Vec<String>>,
    labels: HashMap<String, Vec<String>>,
}

impl Layout {
    fn load_from_disk() -> Layout {
        let mut layout = config::load_keyboard();
        layout.post_load();
        layout
    }

    fn post_load(&mut self) {
        for i in 0..self.key_sizes.len() {
            let row = &self.key_sizes[i];
            let width: f32 = row.iter().sum();
            if (width - self.row_size).abs() > 0.001 {
                panic!(
                    "Row {} has a width of {}, but the row size is {}",
                    i, width, self.row_size
                );
            }
        }

        for i in 0..self.main_layout.len() {
            let row = &self.main_layout[i];
            let width = row.len();
            if width != self.key_sizes[i].len() {
                panic!(
                    "Row {} has {} keys, needs to have {} according to key_sizes",
                    i,
                    width,
                    self.key_sizes[i].len()
                );
            }
        }
    }

    fn label_for_key(&self, key: &str) -> Vec<String> {
        if let Some(label) = self.labels.get(key) {
            return label.clone();
        }
        if key.is_empty() {
            return vec![];
        }
        if key.len() == 1 {
            return vec![key.to_string().to_lowercase()];
        }
        let mut key = key;
        if key.starts_with("KP_") {
            key = &key[3..];
        }
        if key.contains('_') {
            key = key.split('_').next().unwrap();
        }
        vec![format!(
            "{}{}",
            key.chars().next().unwrap().to_uppercase(),
            &key[1..].to_lowercase()
        )]
    }
}

fn key_events_for_macro(macro_verbs: &Vec<String>) -> Vec<(VirtualKey, bool)> {
    let mut key_events = vec![];
    for verb in macro_verbs {
        if let Some(caps) = MACRO_REGEX.captures(verb) {
            if let Ok(virtual_key) = VirtualKey::from_str(&caps[1]) {
                if let Some(state) = caps.get(2) {
                    if state.as_str() == "UP" {
                        key_events.push((virtual_key, false));
                    } else if state.as_str() == "DOWN" {
                        key_events.push((virtual_key, true));
                    } else {
                        error!(
                            "Unknown key state in macro: {}, looking for UP or DOWN.",
                            state.as_str()
                        );
                        return vec![];
                    }
                } else {
                    key_events.push((virtual_key, true));
                    key_events.push((virtual_key, false));
                }
            } else {
                error!("Unknown virtual key: {}", &caps[1]);
                return vec![];
            }
        }
    }
    key_events
}

pub type KeyModifier = u8;
pub const SHIFT: KeyModifier = 0x01;
pub const CAPS_LOCK: KeyModifier = 0x02;
pub const CTRL: KeyModifier = 0x04;
pub const ALT: KeyModifier = 0x08;
pub const NUM_LOCK: KeyModifier = 0x10;
pub const SUPER: KeyModifier = 0x40;
pub const META: KeyModifier = 0x80;

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Clone, Copy, IntegerId, EnumString, EnumIter)]
pub enum VirtualKey {
    Escape = 9,
    N1, // number row
    N2,
    N3,
    N4,
    N5,
    N6,
    N7,
    N8,
    N9,
    N0,
    Minus,
    Plus,
    BackSpace,
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    Oem4, // [ {
    Oem6, // ] }
    Return,
    LCtrl,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Oem1, // ; :
    Oem7, // ' "
    Oem3, // ` ~
    LShift,
    Oem5, // \ |
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,  // , <
    Period, // . >
    Oem2,   // / ?
    RShift,
    KP_Multiply,
    LAlt,
    Space,
    Caps,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    NumLock,
    Scroll,
    KP_7, // KeyPad
    KP_8,
    KP_9,
    KP_Subtract,
    KP_4,
    KP_5,
    KP_6,
    KP_Add,
    KP_1,
    KP_2,
    KP_3,
    KP_0,
    KP_Decimal,
    Oem102 = 94, // Optional key usually between LShift and Z
    F11,
    F12,
    AbntC1,
    Katakana,
    Hiragana,
    Henkan,
    Kana,
    Muhenkan,
    KP_Enter = 104,
    RCtrl,
    KP_Divide,
    Print,
    Meta, // Right Alt aka AltGr
    Home = 110,
    Up,
    Prior,
    Left,
    Right,
    End,
    Down,
    Next,
    Insert,
    Delete,
    XF86AudioMute = 121,
    XF86AudioLowerVolume,
    XF86AudioRaiseVolume,
    Pause = 127,
    AbntC2 = 129,
    Hangul,
    Hanja,
    LSuper = 133,
    RSuper,
    Menu,
    Help = 146,
    XF86MenuKB,
    XF86Sleep = 150,
    XF86Xfer = 155,
    XF86Launch1,
    XF86Launch2,
    XF86WWW,
    XF86Mail = 163,
    XF86Favorites,
    XF86MyComputer,
    XF86Back,
    XF86Forward,
    XF86AudioNext = 171,
    XF86AudioPlay,
    XF86AudioPrev,
    XF86AudioStop,
    XF86HomePage = 180,
    XF86Reload,
    F13 = 191,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    Hyper = 207,
    XF86Launch3,
    XF86Launch4,
    XF86LaunchB,
    XF86Search = 225,
}
