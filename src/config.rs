use crate::config_io;
use crate::keyboard;
use crate::load_with_fallback;
use serde::Deserialize;
use serde::Serialize;

fn def_grab_threshold() -> f32 {
    0.6
}

fn def_scrolling_speed() -> f32 {
    0.6
}

fn def_trigger_threshold() -> f32 {
    0.65
}

fn def_click_freeze_time_ms() -> u32 {
    300
}

fn def_true() -> bool {
    true
}

fn def_false() -> bool {
    false
}

fn def_one() -> f32 {
    1.0
}

#[derive(Deserialize, Serialize)]
pub struct GeneralConfig {
    #[serde(default = "def_grab_threshold")]
    pub grab_threshold: f32,

    #[serde(default = "def_scrolling_speed")]
    pub scrolling_speed: f32,

    #[serde(default = "def_trigger_threshold")]
    pub trigger_threshold: f32,

    #[serde(default = "def_click_freeze_time_ms")]
    pub click_freeze_time_ms: u32,

    #[serde(default = "def_true")]
    pub keyboard_sound_enabled: bool,

    #[serde(default = "def_one")]
    pub keyboard_scale: f32,

    #[serde(default = "def_one")]
    pub desktop_view_scale: f32,

    #[serde(default = "def_one")]
    pub watch_scale: f32,
}

impl GeneralConfig {
    fn panic(msg: &str) {
        panic!("GeneralConfig: {}", msg);
    }

    fn sanitize_range(name: &str, val: f32, from: f32, to: f32) {
        if !val.is_normal() || val < from || val > to {
            panic!(
                "GeneralConfig: {} needs to be between {} and {}",
                name, from, to
            );
        }
    }

    // TODO: config.d/ directory support
    fn load_from_disk() -> GeneralConfig {
        let config = load_general();
        config.post_load();
        config
    }

    fn post_load(&self) {
        GeneralConfig::sanitize_range("grab_threshold", self.grab_threshold, 0.0, 1.0);
        GeneralConfig::sanitize_range("trigger_threshold", self.trigger_threshold, 0.0, 1.0);
        GeneralConfig::sanitize_range("scrolling_speed", self.scrolling_speed, 0.0, 10.0);
        GeneralConfig::sanitize_range("keyboard_scale", self.keyboard_scale, 0.0, 5.0);
        GeneralConfig::sanitize_range("desktop_view_scale", self.desktop_view_scale, 0.0, 5.0);
        GeneralConfig::sanitize_range("watch_scale", self.watch_scale, 0.0, 5.0);
    }
}

pub fn load_keyboard() -> keyboard::Layout {
    let yaml_data = load_with_fallback!("keyboard.yaml", "res/keyboard.yaml");
    serde_yaml::from_str(&yaml_data).expect("Failed to parse keyboard.yaml")
}

pub fn load_general() -> GeneralConfig {
    let yaml_data = load_with_fallback!("config.yaml", "res/config.yaml");
    serde_yaml::from_str(&yaml_data).expect("Failed to parse config.yaml")
}
