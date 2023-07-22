use once_cell::sync::Lazy;
use std::fs;
use std::sync::Mutex;
use std::path::Path;

pub static SESSION: Lazy<Mutex<WlXrSession>> = Lazy::new(|| Mutex::new(WlXrSession::load()));

pub struct WlXrSession {
    pub config_path: String,
    pub screen_flip_h: bool,
    pub screen_flip_v: bool,
    pub screen_invert_color: bool,
}

impl WlXrSession {
    pub fn load() -> WlXrSession {
        let config_path: String;

        if let Ok(home) = std::env::var("HOME") {
            config_path = Path::new(&home)
                .join(".config/wlxroverlay")
                .to_str()
                .unwrap()
                .to_string();
        } else {
            config_path = "/tmp/wlxroverlay".to_string();
            print!("Err: $HOME is not set, using {}", config_path);
        }
        let _ = fs::create_dir(&config_path);

        WlXrSession { config_path, screen_flip_h: false, screen_flip_v: false, screen_invert_color: false, }
    }
}
