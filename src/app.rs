use std::sync::Arc;
use std::time::Duration;

use egui::mutex::Mutex;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::window::WindowId;

use crate::config::{
    self, HotkeyConfig, DEFAULT_OPEN_SETTINGS, DEFAULT_PRIMARY, DEFAULT_SECONDARY,
};
use crate::flash::{self, FlashOverlay};
use crate::gl_window::GlutinWindowContext;

#[derive(Debug)]
pub enum UserEvent {
    Menu(MenuEvent),
    Hotkey(GlobalHotKeyEvent),
    Redraw(Duration),
    EndFlash,
}

pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    hotkey_manager: GlobalHotKeyManager,
    primary: HotKey,
    secondary: HotKey,
    open_settings: HotKey,
    open_id: MenuId,
    quit_id: MenuId,
    tray: Option<TrayIcon>,
    gl_window: Option<GlutinWindowContext>,
    gl: Option<Arc<glow::Context>>,
    egui_glow: Option<egui_glow::EguiGlow>,
    repaint_delay: Duration,
    primary_text: String,
    secondary_text: String,
    open_settings_text: String,
    /// Field values when the settings window was opened (Cancel restores these).
    primary_baseline: String,
    secondary_baseline: String,
    open_settings_baseline: String,
    primary_error: Option<String>,
    secondary_error: Option<String>,
    open_settings_error: Option<String>,
    primary_had_focus: bool,
    secondary_had_focus: bool,
    open_settings_had_focus: bool,
    /// Shown under the Save button when applying shortcuts fails.
    save_message: Option<String>,
    flash: Option<FlashOverlay>,
}

impl App {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let menu_proxy = proxy.clone();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = menu_proxy.send_event(UserEvent::Menu(event));
        }));

        let hotkey_proxy = proxy.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event| {
            let _ = hotkey_proxy.send_event(UserEvent::Hotkey(event));
        }));

        let hotkey_manager = GlobalHotKeyManager::new().expect("failed to create hotkey manager");
        let saved = config::load_hotkeys();
        let (primary, secondary, open_settings, texts) = register_startup_hotkeys(
            &hotkey_manager,
            &saved,
        );

        // First real capture should not pay cold-start cost for the capture backend.
        crate::capture::warm_up();

        Self {
            proxy,
            hotkey_manager,
            primary,
            secondary,
            open_settings,
            open_id: MenuId::new("open"),
            quit_id: MenuId::new("quit"),
            tray: None,
            gl_window: None,
            gl: None,
            egui_glow: None,
            repaint_delay: Duration::MAX,
            primary_text: texts.primary.clone(),
            secondary_text: texts.secondary.clone(),
            open_settings_text: texts.open_settings.clone(),
            primary_baseline: texts.primary,
            secondary_baseline: texts.secondary,
            open_settings_baseline: texts.open_settings,
            primary_error: None,
            secondary_error: None,
            open_settings_error: None,
            primary_had_focus: false,
            secondary_had_focus: false,
            open_settings_had_focus: false,
            save_message: None,
            flash: None,
        }
    }

    fn start_flash(&mut self, rects: &[crate::capture::MonitorRect]) {
        self.flash = None;
        match FlashOverlay::show(rects) {
            Ok(overlay) => {
                self.flash = Some(overlay);
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(flash::FLASH_DURATION);
                    let _ = proxy.send_event(UserEvent::EndFlash);
                });
            }
            Err(_err) => {
                // eprintln!("[flash] {_err}");
            }
        }
    }

    fn handle_active_screen_hotkey(&mut self) {
        match crate::capture::capture_active_monitor() {
            Ok(capture) => {
                // Pixels in RAM → flash now. Stitch/clipboard stay off the UI thread.
                self.start_flash(&capture.rects);
                let parts = capture.parts;
                std::thread::spawn(move || {
                    match crate::capture::finish_clipboard_image(parts) {
                        Ok(()) => {
                            // println!(
                            //     "[active screen] ok (capture={}ms, flash_after={}ms, clipboard={}ms, total={}ms)",
                            //     capture_ms, flash_at_ms, clipboard_ms, total_ms
                            // );
                        }
                        Err(_err) => {
                            // eprintln!("[active screen] {_err}");
                        }
                    }
                });
            }
            Err(_err) => {
                // eprintln!("[active screen] {_err}");
            }
        }
    }

    fn handle_all_screens_hotkey(&mut self) {
        match crate::capture::capture_all_monitors() {
            Ok(capture) => {
                self.start_flash(&capture.rects);
                let parts = capture.parts;
                std::thread::spawn(move || {
                    match crate::capture::finish_clipboard_image(parts) {
                        Ok(()) => {
                            // println!(
                            //     "[all screens] ok (capture={}ms, flash_after={}ms, clipboard={}ms, total={}ms)",
                            //     capture_ms, flash_at_ms, clipboard_ms, total_ms
                            // );
                        }
                        Err(_err) => {
                            // eprintln!("[all screens] {_err}");
                        }
                    }
                });
            }
            Err(_err) => {
                // eprintln!("[all screens] {_err}");
            }
        }
    }

    fn snapshot_settings_baseline(&mut self) {
        self.primary_baseline = self.primary_text.clone();
        self.secondary_baseline = self.secondary_text.clone();
        self.open_settings_baseline = self.open_settings_text.clone();
    }

    fn cancel_settings_edits(&mut self) {
        self.primary_text = self.primary_baseline.clone();
        self.secondary_text = self.secondary_baseline.clone();
        self.open_settings_text = self.open_settings_baseline.clone();
        self.clear_settings_field_state();
        self.save_message = None;
    }

    fn reset_settings_to_defaults(&mut self) {
        self.primary_text = DEFAULT_PRIMARY.to_owned();
        self.secondary_text = DEFAULT_SECONDARY.to_owned();
        self.open_settings_text = DEFAULT_OPEN_SETTINGS.to_owned();
        self.clear_settings_field_state();
        self.save_message = None;
    }

    fn clear_settings_field_state(&mut self) {
        self.primary_error = None;
        self.secondary_error = None;
        self.open_settings_error = None;
        self.primary_had_focus = false;
        self.secondary_had_focus = false;
        self.open_settings_had_focus = false;
    }

    fn can_save_hotkeys(&self) -> bool {
        if self.primary_error.is_some()
            || self.secondary_error.is_some()
            || self.open_settings_error.is_some()
        {
            return false;
        }
        parse_draft_hotkeys(
            &self.primary_text,
            &self.secondary_text,
            &self.open_settings_text,
        )
        .is_ok()
    }

    fn apply_settings_hotkeys(&mut self) -> Result<(), String> {
        let (new_primary, new_secondary, new_open) = parse_draft_hotkeys(
            &self.primary_text,
            &self.secondary_text,
            &self.open_settings_text,
        )?;

        let old = [self.primary, self.secondary, self.open_settings];
        let new = [new_primary, new_secondary, new_open];

        for owned in &old {
            if !new.iter().any(|next| next.id() == owned.id()) {
                let _ = self.hotkey_manager.unregister(*owned);
            }
        }

        for next in &new {
            if old.iter().any(|owned| owned.id() == next.id()) {
                continue;
            }
            if self.hotkey_manager.register(*next).is_err() {
                restore_hotkeys(&self.hotkey_manager, &old, &new);
                return Err("Given hotkey is already taken".to_owned());
            }
        }

        self.primary = new_primary;
        self.secondary = new_secondary;
        self.open_settings = new_open;
        self.snapshot_settings_baseline();
        self.clear_settings_field_state();

        config::save_hotkeys(&HotkeyConfig {
            primary: self.primary_text.clone(),
            secondary: self.secondary_text.clone(),
            open_settings: self.open_settings_text.clone(),
        })
        .map_err(|err| {
            // Hotkeys are already live; still tell the user persistence failed.
            err
        })?;

        Ok(())
    }

    fn open_settings_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.gl_window.is_some() {
            if let Some(gl_window) = self.gl_window.as_ref() {
                gl_window.window().set_minimized(false);
                gl_window.window().set_visible(true);
                gl_window.window().focus_window();
                gl_window.window().request_redraw();
            }
            return;
        }

        self.snapshot_settings_baseline();
        self.save_message = None;

        let gl_window = unsafe { GlutinWindowContext::new(event_loop) };
        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                let s = std::ffi::CString::new(s).expect("CString");
                gl_window.get_proc_address(&s)
            })
        });

        let egui_glow = egui_glow::EguiGlow::new(event_loop, gl.clone(), None, None, true);
        let proxy = Mutex::new(self.proxy.clone());
        egui_glow
            .egui_ctx
            .set_request_repaint_callback(move |info| {
                let _ = proxy.lock().send_event(UserEvent::Redraw(info.delay));
            });

        gl_window.window().set_visible(true);
        gl_window.window().focus_window();
        gl_window.window().request_redraw();

        self.gl = Some(gl);
        self.egui_glow = Some(egui_glow);
        self.gl_window = Some(gl_window);
    }

    fn close_settings_window(&mut self) {
        if let Some(mut egui_glow) = self.egui_glow.take() {
            egui_glow.destroy();
        }
        self.gl = None;
        self.gl_window = None;
        self.repaint_delay = Duration::MAX;
    }

    fn paint_settings(&mut self, event_loop: &ActiveEventLoop) {
        let Some(mut egui_glow) = self.egui_glow.take() else {
            return;
        };
        let Some(gl_window) = self.gl_window.take() else {
            self.egui_glow = Some(egui_glow);
            return;
        };
        let Some(gl) = self.gl.clone() else {
            self.egui_glow = Some(egui_glow);
            self.gl_window = Some(gl_window);
            return;
        };

        egui_glow.run(gl_window.window(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Hotkeys");
                ui.add_space(8.0);

                hotkey_field(
                    ui,
                    "Active screen:",
                    &mut self.primary_text,
                    &mut self.primary_error,
                    &mut self.primary_had_focus,
                    &self.hotkey_manager,
                    &[self.primary, self.secondary, self.open_settings],
                );
                hotkey_field(
                    ui,
                    "All screens:",
                    &mut self.secondary_text,
                    &mut self.secondary_error,
                    &mut self.secondary_had_focus,
                    &self.hotkey_manager,
                    &[self.primary, self.secondary, self.open_settings],
                );
                hotkey_field(
                    ui,
                    "Open settings:",
                    &mut self.open_settings_text,
                    &mut self.open_settings_error,
                    &mut self.open_settings_had_focus,
                    &self.hotkey_manager,
                    &[self.primary, self.secondary, self.open_settings],
                );

                ui.add_space(12.0);
                let can_save = self.can_save_hotkeys();
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(can_save, |ui| {
                        if ui.button("Save").clicked() {
                            match self.apply_settings_hotkeys() {
                                Ok(()) => {
                                    self.save_message = None;
                                    // println!("[settings] saved");
                                    // println!("  primary: {}", self.primary_text);
                                    // println!("  secondary: {}", self.secondary_text);
                                    // println!("  open settings: {}", self.open_settings_text);
                                }
                                Err(message) => {
                                    self.save_message = Some(message);
                                }
                            }
                        }
                    });
                    if ui.button("Cancel").clicked() {
                        self.cancel_settings_edits();
                    }
                    if ui.button("Reset to default").clicked() {
                        self.reset_settings_to_defaults();
                    }
                });

                if let Some(message) = self.save_message.as_ref() {
                    ui.colored_label(egui::Color32::from_rgb(180, 40, 40), message);
                }
            });
        });

        unsafe {
            use glow::HasContext as _;
            gl.clear_color(0.70, 0.70, 0.70, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
        egui_glow.paint(gl_window.window());
        let _ = gl_window.swap_buffers();
        gl_window.window().set_visible(true);

        if self.repaint_delay.is_zero() {
            gl_window.window().request_redraw();
            event_loop.set_control_flow(ControlFlow::Poll);
        } else if let Some(when) = std::time::Instant::now().checked_add(self.repaint_delay) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(when));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }

        self.egui_glow = Some(egui_glow);
        self.gl_window = Some(gl_window);
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) && self.tray.is_none() {
            self.tray = Some(build_tray_icon(&self.open_id, &self.quit_id));
        }

        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            if let Some(gl_window) = self.gl_window.as_ref() {
                gl_window.window().request_redraw();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Redraw(delay) => {
                self.repaint_delay = delay;
                if let Some(gl_window) = self.gl_window.as_ref() {
                    gl_window.window().request_redraw();
                }
            }
            UserEvent::EndFlash => {
                self.flash = None;
            }
            UserEvent::Menu(menu_event) => {
                if menu_event.id == self.open_id {
                    self.open_settings_window(event_loop);
                } else if menu_event.id == self.quit_id {
                    self.flash = None;
                    self.close_settings_window();
                    drop(self.tray.take());
                    event_loop.exit();
                }
            }
            UserEvent::Hotkey(hotkey_event) => {
                if hotkey_event.state != HotKeyState::Pressed {
                    return;
                }
                if hotkey_event.id == self.primary.id() {
                    self.handle_active_screen_hotkey();
                } else if hotkey_event.id == self.secondary.id() {
                    self.handle_all_screens_hotkey();
                } else if hotkey_event.id == self.open_settings.id() {
                    // println!("[settings] {}", self.open_settings_text);
                    self.open_settings_window(event_loop);
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match &event {
            WindowEvent::CloseRequested => {
                self.close_settings_window();
                event_loop.set_control_flow(ControlFlow::Wait);
                return;
            }
            WindowEvent::Destroyed => return,
            WindowEvent::RedrawRequested => {
                self.paint_settings(event_loop);
                return;
            }
            WindowEvent::Resized(size) => {
                if let Some(gl_window) = self.gl_window.as_ref() {
                    gl_window.resize(*size);
                }
            }
            _ => {}
        }

        let Some(egui_glow) = self.egui_glow.as_mut() else {
            return;
        };
        let Some(gl_window) = self.gl_window.as_ref() else {
            return;
        };

        let response = egui_glow.on_window_event(gl_window.window(), &event);
        if response.repaint {
            gl_window.window().request_redraw();
        }
    }
}

fn hotkey_field(
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
        // Track focus ourselves: egui's lost_focus() can miss clicks between fields.
        if *had_focus && !has_focus {
            *error = probe_hotkey(manager, live, text);
        }
        *had_focus = has_focus;
    });

    if let Some(message) = error.as_ref() {
        ui.colored_label(egui::Color32::from_rgb(180, 40, 40), message);
    }
}

fn parse_draft_hotkeys(
    primary_text: &str,
    secondary_text: &str,
    open_settings_text: &str,
) -> Result<(HotKey, HotKey, HotKey), String> {
    let primary: HotKey = primary_text
        .parse()
        .map_err(|_| "Given hotkey is invalid".to_owned())?;
    let secondary: HotKey = secondary_text
        .parse()
        .map_err(|_| "Given hotkey is invalid".to_owned())?;
    let open_settings: HotKey = open_settings_text
        .parse()
        .map_err(|_| "Given hotkey is invalid".to_owned())?;

    if primary.id() == secondary.id()
        || primary.id() == open_settings.id()
        || secondary.id() == open_settings.id()
    {
        return Err("Given hotkey is already taken".to_owned());
    }

    Ok((primary, secondary, open_settings))
}

/// Probe whether a shortcut string can be registered by the OS.
///
/// Does not change the app's live hotkeys: on success we unregister the
/// probe immediately. Shortcuts we already own count as valid.
fn probe_hotkey(manager: &GlobalHotKeyManager, live: &[HotKey], text: &str) -> Option<String> {
    let hotkey: HotKey = match text.parse() {
        Ok(hotkey) => hotkey,
        Err(_) => return Some("Given hotkey is invalid".to_owned()),
    };

    if live.iter().any(|owned| owned.id() == hotkey.id()) {
        return None;
    }

    match manager.register(hotkey) {
        Ok(()) => {
            let _ = manager.unregister(hotkey);
            None
        }
        Err(_) => Some("Given hotkey is already taken".to_owned()),
    }
}

fn restore_hotkeys(manager: &GlobalHotKeyManager, old: &[HotKey; 3], new: &[HotKey; 3]) {
    for next in new {
        if !old.iter().any(|owned| owned.id() == next.id()) {
            let _ = manager.unregister(*next);
        }
    }
    for owned in old {
        let _ = manager.register(*owned);
    }
}

fn register_startup_hotkeys(
    manager: &GlobalHotKeyManager,
    saved: &HotkeyConfig,
) -> (HotKey, HotKey, HotKey, HotkeyConfig) {
    if let Ok((primary, secondary, open_settings)) = parse_draft_hotkeys(
        &saved.primary,
        &saved.secondary,
        &saved.open_settings,
    ) {
        if manager.register(primary).is_ok()
            && manager.register(secondary).is_ok()
            && manager.register(open_settings).is_ok()
        {
            return (primary, secondary, open_settings, saved.clone());
        }
        let _ = manager.unregister(primary);
        let _ = manager.unregister(secondary);
        let _ = manager.unregister(open_settings);
    }

    let defaults = HotkeyConfig::default();
    let primary = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F2);
    let secondary = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F3);
    let open_settings = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F4);
    manager
        .register(primary)
        .expect("failed to register default Ctrl+Shift+F2");
    manager
        .register(secondary)
        .expect("failed to register default Ctrl+Shift+F3");
    manager
        .register(open_settings)
        .expect("failed to register default Ctrl+Shift+F4");
    (primary, secondary, open_settings, defaults)
}

fn build_tray_icon(open_id: &MenuId, quit_id: &MenuId) -> TrayIcon {
    let menu = Menu::new();
    let open = MenuItem::with_id(open_id.clone(), "Open settings", true, None);
    let quit = MenuItem::with_id(quit_id.clone(), "Quit", true, None);
    menu.append(&open).expect("failed to add Open");
    menu.append(&PredefinedMenuItem::separator())
        .expect("failed to add separator");
    menu.append(&quit).expect("failed to add Quit");

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("simple-capture")
        .with_icon(load_tray_icon())
        .build()
        .expect("failed to create tray icon")
}

fn load_tray_icon() -> Icon {
    let bytes = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(bytes)
        .expect("tray icon PNG is invalid")
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).expect("invalid tray icon") 
}
