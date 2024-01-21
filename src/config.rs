use log::error;
use std::{
    fs::{self, create_dir},
    path::Path,
};

use crate::keyboard;

pub fn get_config_root() -> String {
    let config_path: String;

    if let Ok(home) = std::env::var("HOME") {
        config_path = Path::new(&home)
            .join(".config/wlxroverlay")
            .to_str()
            .unwrap()
            .to_string();
    } else {
        config_path = "/tmp/wlxroverlay".to_string();
        error!("Err: $HOME is not set, using {}", config_path);
    }
    // Make sure config directory is created
    let _ = create_dir(&config_path);
    config_path
}

fn get_config_path(filename: &str) -> String {
    format!("{}/{}", get_config_root().as_str(), filename)
}

fn load(filename: &str) -> Option<String> {
    if let Ok(data) = fs::read_to_string(get_config_path(filename)) {
        Some(data)
    } else {
        None
    }
}

macro_rules! load_with_fallback {
    ($filename: expr,  $fallback: expr) => {
        if let Some(data) = load($filename) {
            println!("Loading config {}/{}", get_config_root(), $filename);
            data
        } else {
            println!(
                "Config {}/{} does not exist, using fallback",
                get_config_root(),
                $filename
            );
            include_str!($fallback).to_string()
        }
    };
}

pub fn load_keyboard() -> keyboard::Layout {
    let yaml_data = load_with_fallback!("keyboard.yaml", "res/keyboard.yaml");
    serde_yaml::from_str(&yaml_data).expect("Failed to parse keyboard.yaml")
}
