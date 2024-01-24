use log::error;
use once_cell::sync::Lazy;
use std::{
    fs::{self, create_dir},
    path::PathBuf,
};

const FALLBACK_CONFIG_PATH: &str = "/tmp/wlxroverlay";

pub static CONFIG_ROOT_PATH: Lazy<PathBuf> = Lazy::new(|| {
    if let Ok(xdg_dirs) = xdg::BaseDirectories::new() {
        if let Some(dir) = xdg_dirs.get_config_dirs().first() {
            return dir.clone().join("wlxoverlay");
        }
    }
    //Return fallback config path
    error!(
        "Err: Failed to find config path, using {}",
        FALLBACK_CONFIG_PATH
    );
    PathBuf::from(FALLBACK_CONFIG_PATH)
});

// Make sure config directory is present and return config path
pub fn ensure_config_root() -> PathBuf {
    let path = CONFIG_ROOT_PATH.clone();
    let _ = create_dir(&path);
    path
}

fn get_config_file_path(filename: &str) -> PathBuf {
    let mut config_root = CONFIG_ROOT_PATH.clone();
    config_root.push(filename);
    config_root
}

pub fn load(filename: &str) -> Option<String> {
    let path = get_config_file_path(filename);
    println!("Loading config {}", path.to_string_lossy());

    if let Ok(data) = fs::read_to_string(path) {
        Some(data)
    } else {
        None
    }
}

#[macro_export]
macro_rules! load_with_fallback {
    ($filename: expr,  $fallback: expr) => {
        if let Some(data) = config_io::load($filename) {
            data
        } else {
            println!(
                "Config {}/{} does not exist, using internal fallback",
                config_io::CONFIG_ROOT_PATH.to_string_lossy(),
                $filename
            );
            include_str!($fallback).to_string()
        }
    };
}