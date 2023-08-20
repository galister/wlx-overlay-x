use crate::overlay::OverlayData;
use idmap::{IdMap, idmap};
use idmap_derive::IntegerId;
use once_cell::sync::Lazy;

static KEYS_TO_MODS: Lazy<IdMap<VirtualKey, KeyModifier>> = Lazy::new(|| { idmap! {
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
} });

static MODS_TO_KEYS: Lazy<IdMap<KeyModifier, Vec<VirtualKey>>> = Lazy::new(|| { idmap! {
    SHIFT => vec![VirtualKey::LShift, VirtualKey::RShift],
    CAPS_LOCK => vec![VirtualKey::Caps],
    CTRL => vec![VirtualKey::LCtrl, VirtualKey::RCtrl],
    ALT => vec![VirtualKey::LAlt],
    NUM_LOCK => vec![VirtualKey::NumLock],
    SUPER => vec![VirtualKey::LSuper, VirtualKey::RSuper],
    META => vec![VirtualKey::Meta],
} });

pub fn create_keyboard() -> OverlayData {

    OverlayData { 
        name: "Kbd".to_string(),
        ..Default::default()
    }
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
#[derive(Debug, PartialEq, Clone, Copy, IntegerId)]
enum VirtualKey
{
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
    Comma, // , <
    Period, // . >
    Oem2, // / ?
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
