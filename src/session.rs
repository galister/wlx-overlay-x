use once_cell::sync::Lazy;
use std::fs;
use std::{path::Path, sync::Arc};

pub static SESSION: Lazy<Arc<WlXrSession>> = Lazy::new(|| Arc::new(WlXrSession::load()));

pub struct WlXrSession {
    pub config_path: String,
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

        WlXrSession { config_path }
    }
}
