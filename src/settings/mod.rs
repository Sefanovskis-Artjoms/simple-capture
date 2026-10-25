//! Settings window UI state (egui).

use global_hotkey::hotkey::HotKey;
use global_hotkey::GlobalHotKeyManager;

use crate::config::{
    self, AppConfig, HotkeyConfig, ModeConfig, DEFAULT_ACTIVE, DEFAULT_ALL_SCREENS,
    DEFAULT_OPEN_SETTINGS, DEFAULT_REGION_PRIMARY, DEFAULT_REGION_SECONDARY,
};
use crate::hotkeys;

pub struct SettingsUi {
    pub active_text: String,
    pub all_text: String,
    pub region_primary_text: String,
    pub region_secondary_text: String,
    pub open_settings_text: String,
    active_baseline: String,
    all_baseline: String,
    region_primary_baseline: String,
    region_secondary_baseline: String,
    open_settings_baseline: String,
    pub active_error: Option<String>,
    pub all_error: Option<String>,
    pub region_primary_error: Option<String>,
    pub region_secondary_error: Option<String>,
    pub open_settings_error: Option<String>,
    active_had_focus: bool,
    all_had_focus: bool,
    region_primary_had_focus: bool,
    region_secondary_had_focus: bool,
    open_settings_had_focus: bool,
    pub save_message: Option<String>,
    pub swap_region_modes: bool,
    pub disable_secondary_editor: bool,
}

impl SettingsUi {
    pub fn from_config(cfg: &AppConfig) -> Self {
        Self {
            active_text: cfg.hotkeys.active_screen.clone(),
            all_text: cfg.hotkeys.all_screens.clone(),
            region_primary_text: cfg.hotkeys.region_primary.clone(),
            region_secondary_text: cfg.hotkeys.region_secondary.clone(),
            open_settings_text: cfg.hotkeys.open_settings.clone(),
            active_baseline: cfg.hotkeys.active_screen.clone(),
            all_baseline: cfg.hotkeys.all_screens.clone(),
            region_primary_baseline: cfg.hotkeys.region_primary.clone(),
            region_secondary_baseline: cfg.hotkeys.region_secondary.clone(),
            open_settings_baseline: cfg.hotkeys.open_settings.clone(),
            active_error: None,
            all_error: None,
            region_primary_error: None,
            region_secondary_error: None,
            open_settings_error: None,
            active_had_focus: false,
            all_had_focus: false,
            region_primary_had_focus: false,
            region_secondary_had_focus: false,
            open_settings_had_focus: false,
            save_message: None,
            swap_region_modes: cfg.modes.swap_region_modes,
            disable_secondary_editor: cfg.modes.disable_secondary_editor,
        }
    }

    pub fn snapshot_baseline(&mut self) {
        self.active_baseline = self.active_text.clone();
        self.all_baseline = self.all_text.clone();
        self.region_primary_baseline = self.region_primary_text.clone();
        self.region_secondary_baseline = self.region_secondary_text.clone();
        self.open_settings_baseline = self.open_settings_text.clone();
    }

    pub fn cancel(&mut self) {
        self.active_text = self.active_baseline.clone();
        self.all_text = self.all_baseline.clone();
        self.region_primary_text = self.region_primary_baseline.clone();
        self.region_secondary_text = self.region_secondary_baseline.clone();
        self.open_settings_text = self.open_settings_baseline.clone();
        self.clear_field_state();
        self.save_message = None;
    }

    pub fn reset_defaults(&mut self) {
        self.active_text = DEFAULT_ACTIVE.to_owned();
        self.all_text = DEFAULT_ALL_SCREENS.to_owned();
        self.region_primary_text = DEFAULT_REGION_PRIMARY.to_owned();
        self.region_secondary_text = DEFAULT_REGION_SECONDARY.to_owned();
        self.open_settings_text = DEFAULT_OPEN_SETTINGS.to_owned();
        self.swap_region_modes = false;
        self.disable_secondary_editor = false;
        self.clear_field_state();
        self.save_message = None;
    }

    fn clear_field_state(&mut self) {
        self.active_error = None;
        self.all_error = None;
        self.region_primary_error = None;
        self.region_secondary_error = None;
        self.open_settings_error = None;
        self.active_had_focus = false;
        self.all_had_focus = false;
        self.region_primary_had_focus = false;
        self.region_secondary_had_focus = false;
        self.open_settings_had_focus = false;
    }

    pub fn draft_hotkeys(&self) -> HotkeyConfig {
        HotkeyConfig {
            active_screen: self.active_text.clone(),
            all_screens: self.all_text.clone(),
            region_primary: self.region_primary_text.clone(),
            region_secondary: self.region_secondary_text.clone(),
            open_settings: self.open_settings_text.clone(),
        }
    }

    pub fn draft_modes(&self) -> ModeConfig {
        ModeConfig {
            swap_region_modes: self.swap_region_modes,
            disable_secondary_editor: self.disable_secondary_editor,
        }
    }

    /// Paint settings and optionally return newly applied hotkey set.
    pub fn can_save(&self) -> bool {
        if self.active_error.is_some()
            || self.all_error.is_some()
            || self.region_primary_error.is_some()
            || self.region_secondary_error.is_some()
            || self.open_settings_error.is_some()
        {
            return false;
        }
        hotkeys::parse_all(&self.draft_hotkeys()).is_ok()
    }
}

fn hotkey_row(
    ui: &mut egui::Ui,
    label: &str,
    text: &mut String,
    error: &mut Option<String>,
    had_focus: &mut bool,
    manager: &GlobalHotKeyManager,
    live: &[HotKey],
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let response = ui.add(egui::TextEdit::singleline(text).desired_width(220.0));
        let has_focus = response.has_focus();
        if *had_focus && !has_focus {
            *error = hotkeys::probe(manager, live, text);
        }
        *had_focus = has_focus;
    });
    if let Some(message) = error.as_ref() {
        ui.colored_label(egui::Color32::from_rgb(180, 40, 40), message);
    }
}

/// Paint settings and optionally return newly applied hotkey set.
pub fn paint(
    ui_state: &mut SettingsUi,
    ctx: &egui::Context,
    manager: &GlobalHotKeyManager,
    live: &[HotKey; 5],
) -> Option<([HotKey; 5], AppConfig)> {
    let mut result = None;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Hotkeys");
        ui.add_space(6.0);

        hotkey_row(
            ui,
            "Active screen:",
            &mut ui_state.active_text,
            &mut ui_state.active_error,
            &mut ui_state.active_had_focus,
            manager,
            live,
        );
        hotkey_row(
            ui,
            "All screens:",
            &mut ui_state.all_text,
            &mut ui_state.all_error,
            &mut ui_state.all_had_focus,
            manager,
            live,
        );
        hotkey_row(
            ui,
            "Region (primary):",
            &mut ui_state.region_primary_text,
            &mut ui_state.region_primary_error,
            &mut ui_state.region_primary_had_focus,
            manager,
            live,
        );
        hotkey_row(
            ui,
            "Region (secondary):",
            &mut ui_state.region_secondary_text,
            &mut ui_state.region_secondary_error,
            &mut ui_state.region_secondary_had_focus,
            manager,
            live,
        );
        hotkey_row(
            ui,
            "Open settings:",
            &mut ui_state.open_settings_text,
            &mut ui_state.open_settings_error,
            &mut ui_state.open_settings_had_focus,
            manager,
            live,
        );

        ui.add_space(12.0);
        ui.heading("Modes");
        ui.checkbox(
            &mut ui_state.swap_region_modes,
            "Swap region modes (secondary hotkey uses Flameshot-style)",
        );
        ui.checkbox(
            &mut ui_state.disable_secondary_editor,
            "Disable secondary editor (copy only, like Snipping Tool)",
        );

        ui.add_space(12.0);
        let can_save = ui_state.can_save();
        ui.horizontal(|ui| {
            ui.add_enabled_ui(can_save, |ui| {
                if ui.button("Save").clicked() {
                    match try_save(ui_state, manager, live) {
                        Ok((keys, cfg)) => {
                            ui_state.save_message = None;
                            result = Some((keys, cfg));
                        }
                        Err(msg) => ui_state.save_message = Some(msg),
                    }
                }
            });
            if ui.button("Cancel").clicked() {
                ui_state.cancel();
            }
            if ui.button("Reset to default").clicked() {
                ui_state.reset_defaults();
            }
        });

        if let Some(message) = ui_state.save_message.as_ref() {
            ui.colored_label(egui::Color32::from_rgb(180, 40, 40), message);
        }
    });

    result
}

fn try_save(
    ui_state: &mut SettingsUi,
    manager: &GlobalHotKeyManager,
    live: &[HotKey; 5],
) -> Result<([HotKey; 5], AppConfig), String> {
    let draft = ui_state.draft_hotkeys();
    let new = hotkeys::parse_all(&draft)?;
    hotkeys::apply(manager, live, &new)?;
    ui_state.snapshot_baseline();
    ui_state.clear_field_state();
    let cfg = AppConfig {
        hotkeys: draft,
        modes: ui_state.draft_modes(),
    };
    config::save(&cfg)?;
    Ok((new, cfg))
}
