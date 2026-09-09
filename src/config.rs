//! Persisted hotkey settings (JSON in the OS config directory).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_PRIMARY: &str = "Ctrl+Shift+F2";
pub const DEFAULT_SECONDARY: &str = "Ctrl+Shift+F3";
pub const DEFAULT_OPEN_SETTINGS: &str = "Ctrl+Shift+F4";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub primary: String,
    pub secondary: String,
    pub open_settings: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            primary: DEFAULT_PRIMARY.to_owned(),
            secondary: DEFAULT_SECONDARY.to_owned(),
            open_settings: DEFAULT_OPEN_SETTINGS.to_owned(),
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "simple-capture", "simple-capture")
        .map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn load_hotkeys() -> HotkeyConfig {
    let Some(path) = config_dir().map(|dir| dir.join("hotkeys.json")) else {
        return HotkeyConfig::default();
    };
    let Ok(bytes) = fs::read_to_string(&path) else {
        return HotkeyConfig::default();
    };
    serde_json::from_str(&bytes).unwrap_or_default()
}

pub fn save_hotkeys(config: &HotkeyConfig) -> Result<(), String> {
    let dir = config_dir().ok_or_else(|| "Could not find config directory".to_owned())?;
    fs::create_dir_all(&dir).map_err(|_| "Could not create config directory".to_owned())?;
    let path = dir.join("hotkeys.json");
    let json = serde_json::to_string_pretty(config)
        .map_err(|_| "Could not serialize settings".to_owned())?;
    fs::write(&path, json).map_err(|_| "Could not write settings file".to_owned())?;
    Ok(())
}
