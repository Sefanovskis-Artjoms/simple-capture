//! Persisted settings (hotkeys + capture mode options).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_ACTIVE: &str = "Ctrl+Shift+F2";
pub const DEFAULT_ALL_SCREENS: &str = "Ctrl+Shift+F3";
pub const DEFAULT_REGION_PRIMARY: &str = "Ctrl+Shift+F5";
pub const DEFAULT_REGION_SECONDARY: &str = "Ctrl+Shift+F6";
pub const DEFAULT_OPEN_SETTINGS: &str = "Ctrl+Shift+F4";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    #[serde(default = "default_active", alias = "primary")]
    pub active_screen: String,
    #[serde(default = "default_all", alias = "secondary")]
    pub all_screens: String,
    #[serde(default = "default_region_primary", alias = "region")]
    pub region_primary: String,
    #[serde(default = "default_region_secondary")]
    pub region_secondary: String,
    pub open_settings: String,
}

fn default_active() -> String {
    DEFAULT_ACTIVE.to_owned()
}
fn default_all() -> String {
    DEFAULT_ALL_SCREENS.to_owned()
}
fn default_region_primary() -> String {
    DEFAULT_REGION_PRIMARY.to_owned()
}
fn default_region_secondary() -> String {
    DEFAULT_REGION_SECONDARY.to_owned()
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            active_screen: DEFAULT_ACTIVE.to_owned(),
            all_screens: DEFAULT_ALL_SCREENS.to_owned(),
            region_primary: DEFAULT_REGION_PRIMARY.to_owned(),
            region_secondary: DEFAULT_REGION_SECONDARY.to_owned(),
            open_settings: DEFAULT_OPEN_SETTINGS.to_owned(),
        }
    }
}

/// Mode switches from AGENTS.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeConfig {
    /// When true, the “region secondary” hotkey runs the Flameshot-style (primary) flow.
    #[serde(default)]
    pub swap_region_modes: bool,
    /// When true, secondary flow only copies (no editor window).
    #[serde(default)]
    pub disable_secondary_editor: bool,
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            swap_region_modes: false,
            disable_secondary_editor: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub modes: ModeConfig,
}

fn config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "simple-capture", "simple-capture")
        .map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn load() -> AppConfig {
    // Prefer unified settings.json; fall back to legacy hotkeys.json.
    if let Some(path) = config_dir().map(|d| d.join("settings.json")) {
        if let Ok(bytes) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&bytes) {
                return cfg;
            }
        }
    }
    if let Some(path) = config_dir().map(|d| d.join("hotkeys.json")) {
        if let Ok(bytes) = fs::read_to_string(&path) {
            if let Ok(hotkeys) = serde_json::from_str::<HotkeyConfig>(&bytes) {
                return AppConfig {
                    hotkeys,
                    modes: ModeConfig::default(),
                };
            }
        }
    }
    AppConfig::default()
}

pub fn save(config: &AppConfig) -> Result<(), String> {
    let dir = config_dir().ok_or_else(|| "Could not find config directory".to_owned())?;
    fs::create_dir_all(&dir).map_err(|_| "Could not create config directory".to_owned())?;
    let path = dir.join("settings.json");
    let json = serde_json::to_string_pretty(config)
        .map_err(|_| "Could not serialize settings".to_owned())?;
    fs::write(&path, json).map_err(|_| "Could not write settings file".to_owned())?;
    Ok(())
}

/// Resolve which region hotkey should use Flameshot-style vs ShareX-style.
pub fn resolve_region_flow(modes: &ModeConfig, is_secondary_hotkey: bool) -> RegionFlow {
    let use_primary = if modes.swap_region_modes {
        is_secondary_hotkey
    } else {
        !is_secondary_hotkey
    };
    if use_primary {
        RegionFlow::Primary
    } else {
        RegionFlow::Secondary {
            open_editor: !modes.disable_secondary_editor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionFlow {
    /// Freeze → select → draw → Ctrl+C copies.
    Primary,
    /// Select → copy immediately → optional editor.
    Secondary { open_editor: bool },
}
