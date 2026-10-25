//! Thin ApplicationHandler orchestration.

mod events;

pub use events::UserEvent;

use std::sync::Arc;
use std::time::Duration;

use egui::mutex::Mutex;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tray_icon::menu::{MenuEvent, MenuId};
use tray_icon::TrayIcon;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::window::WindowId;

use crate::config::{self, AppConfig, resolve_region_flow};
use crate::gl_window::GlutinWindowContext;
use crate::hotkeys::{self, RegisteredHotkeys};
use crate::settings::{self, SettingsUi};
use crate::tray;

#[cfg(feature = "flash")]
use crate::flash::FlashOverlay;

#[cfg(feature = "region")]
use crate::region::{RegionSession, SessionOutcome};

#[cfg(feature = "editor")]
use crate::editor::EditorWindow;

pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    hotkey_manager: GlobalHotKeyManager,
    keys: RegisteredHotkeys,
    config: AppConfig,
    open_id: MenuId,
    quit_id: MenuId,
    tray: Option<TrayIcon>,
    settings_ui: SettingsUi,
    settings_gl: Option<GlutinWindowContext>,
    settings_glow: Option<Arc<glow::Context>>,
    settings_egui: Option<egui_glow::EguiGlow>,
    repaint_delay: Duration,
    #[cfg(feature = "flash")]
    flash: Option<FlashOverlay>,
    #[cfg(feature = "region")]
    region: Option<RegionSession>,
    #[cfg(feature = "editor")]
    editor: Option<EditorWindow>,
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
        let config = config::load();
        let keys = hotkeys::register_startup(&hotkey_manager, &config.hotkeys);
        let settings_ui = SettingsUi::from_config(&AppConfig {
            hotkeys: keys.texts.clone(),
            modes: config.modes.clone(),
        });

        crate::capture::warm_up();

        Self {
            proxy,
            hotkey_manager,
            keys,
            config,
            open_id: MenuId::new("open"),
            quit_id: MenuId::new("quit"),
            tray: None,
            settings_ui,
            settings_gl: None,
            settings_glow: None,
            settings_egui: None,
            repaint_delay: Duration::MAX,
            #[cfg(feature = "flash")]
            flash: None,
            #[cfg(feature = "region")]
            region: None,
            #[cfg(feature = "editor")]
            editor: None,
        }
    }

    fn live_keys(&self) -> [HotKey; 5] {
        self.keys.as_array()
    }

    fn set_keys_from_array(&mut self, keys: [HotKey; 5], cfg: AppConfig) {
        self.keys.active_screen = keys[0];
        self.keys.all_screens = keys[1];
        self.keys.region_primary = keys[2];
        self.keys.region_secondary = keys[3];
        self.keys.open_settings = keys[4];
        self.keys.texts = cfg.hotkeys.clone();
        self.config = cfg;
    }

    #[cfg(feature = "flash")]
    fn start_flash(&mut self, rects: &[crate::capture::MonitorRect]) {
        self.flash = None;
        if let Ok(overlay) = FlashOverlay::show(rects) {
            self.flash = Some(overlay);
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(crate::flash::FLASH_DURATION);
                let _ = proxy.send_event(UserEvent::EndFlash);
            });
        }
    }

    fn handle_active_screen(&mut self) {
        match crate::capture::capture_active_monitor() {
            Ok(capture) => {
                #[cfg(feature = "flash")]
                self.start_flash(&capture.rects);
                let parts = capture.parts;
                std::thread::spawn(move || {
                    let _ = crate::capture::finish_clipboard_image(parts);
                });
            }
            Err(_) => {}
        }
    }

    fn handle_all_screens(&mut self) {
        match crate::capture::capture_all_monitors() {
            Ok(capture) => {
                #[cfg(feature = "flash")]
                self.start_flash(&capture.rects);
                let parts = capture.parts;
                std::thread::spawn(move || {
                    let _ = crate::capture::finish_clipboard_image(parts);
                });
            }
            Err(_) => {}
        }
    }

    #[cfg(feature = "region")]
    fn handle_region(&mut self, event_loop: &ActiveEventLoop, secondary_hotkey: bool) {
        if self.region.is_some() {
            return;
        }
        #[cfg(feature = "flash")]
        {
            self.flash = None;
        }

        let flow = resolve_region_flow(&self.config.modes, secondary_hotkey);
        match crate::capture::capture_all_monitors() {
            Ok(capture) => match RegionSession::start(event_loop, capture.parts, flow) {
                Ok(session) => {
                    self.region = Some(session);
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
                Err(_) => {}
            },
            Err(_) => {}
        }
    }

    #[cfg(feature = "region")]
    fn finish_region(&mut self, event_loop: &ActiveEventLoop, outcome: SessionOutcome) {
        self.region = None;
        event_loop.set_control_flow(ControlFlow::Wait);

        match outcome {
            SessionOutcome::Cancelled => {}
            SessionOutcome::PrimaryCopied { image }
            | SessionOutcome::SecondaryReady {
                image,
                open_editor: false,
            } => {
                std::thread::spawn(move || {
                    let _ = crate::clipboard::copy_bgra_to_clipboard(image);
                });
            }
            SessionOutcome::SecondaryReady {
                image,
                open_editor: true,
            } => {
                let for_clipboard = image.clone();
                std::thread::spawn(move || {
                    let _ = crate::clipboard::copy_bgra_to_clipboard(for_clipboard);
                });
                #[cfg(feature = "editor")]
                {
                    if let Ok(ed) = EditorWindow::open(event_loop, self.proxy.clone(), image) {
                        self.editor = Some(ed);
                    }
                }
                #[cfg(not(feature = "editor"))]
                {
                    let _ = image;
                }
            }
        }
    }

    fn open_settings(&mut self, event_loop: &ActiveEventLoop) {
        if self.settings_gl.is_some() {
            if let Some(gl) = self.settings_gl.as_ref() {
                gl.window().set_minimized(false);
                gl.window().set_visible(true);
                gl.window().focus_window();
                gl.window().request_redraw();
            }
            return;
        }

        self.settings_ui.snapshot_baseline();
        self.settings_ui.save_message = None;

        let gl_window =
            unsafe { GlutinWindowContext::new(event_loop, "simple-capture — Settings", 520.0, 420.0) };
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

        self.settings_glow = Some(gl);
        self.settings_egui = Some(egui_glow);
        self.settings_gl = Some(gl_window);
    }

    fn close_settings(&mut self) {
        if let Some(mut egui) = self.settings_egui.take() {
            egui.destroy();
        }
        self.settings_glow = None;
        self.settings_gl = None;
        self.repaint_delay = Duration::MAX;
    }

    fn paint_settings(&mut self, event_loop: &ActiveEventLoop) {
        let Some(mut egui_glow) = self.settings_egui.take() else {
            return;
        };
        let Some(gl_window) = self.settings_gl.take() else {
            self.settings_egui = Some(egui_glow);
            return;
        };
        let Some(gl) = self.settings_glow.clone() else {
            self.settings_egui = Some(egui_glow);
            self.settings_gl = Some(gl_window);
            return;
        };

        let live = self.live_keys();
        let mut applied = None;
        egui_glow.run(gl_window.window(), |ctx| {
            applied = settings::paint(&mut self.settings_ui, ctx, &self.hotkey_manager, &live);
        });
        if let Some((keys, cfg)) = applied {
            self.set_keys_from_array(keys, cfg);
        }

        unsafe {
            use glow::HasContext as _;
            gl.clear_color(0.70, 0.70, 0.70, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
        egui_glow.paint(gl_window.window());
        let _ = gl_window.swap_buffers();

        if self.repaint_delay.is_zero() {
            gl_window.window().request_redraw();
            event_loop.set_control_flow(ControlFlow::Poll);
        } else if let Some(when) = std::time::Instant::now().checked_add(self.repaint_delay) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(when));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }

        self.settings_egui = Some(egui_glow);
        self.settings_gl = Some(gl_window);
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) && self.tray.is_none() {
            self.tray = Some(tray::build(&self.open_id, &self.quit_id));
        }
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            if let Some(gl) = self.settings_gl.as_ref() {
                gl.window().request_redraw();
            }
            #[cfg(feature = "editor")]
            if let Some(ed) = self.editor.as_ref() {
                ed.request_redraw();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Redraw(delay) => {
                self.repaint_delay = delay;
                if let Some(gl) = self.settings_gl.as_ref() {
                    gl.window().request_redraw();
                }
                #[cfg(feature = "editor")]
                if let Some(ed) = self.editor.as_ref() {
                    ed.request_redraw();
                }
            }
            #[cfg(feature = "flash")]
            UserEvent::EndFlash => {
                self.flash = None;
            }
            #[cfg(not(feature = "flash"))]
            UserEvent::EndFlash => {}
            UserEvent::Menu(menu_event) => {
                if menu_event.id == self.open_id {
                    self.open_settings(event_loop);
                } else if menu_event.id == self.quit_id {
                    #[cfg(feature = "flash")]
                    {
                        self.flash = None;
                    }
                    #[cfg(feature = "region")]
                    {
                        self.region = None;
                    }
                    #[cfg(feature = "editor")]
                    if let Some(mut ed) = self.editor.take() {
                        ed.destroy();
                    }
                    self.close_settings();
                    drop(self.tray.take());
                    event_loop.exit();
                }
            }
            UserEvent::Hotkey(hotkey_event) => {
                if hotkey_event.state != HotKeyState::Pressed {
                    return;
                }
                #[cfg(feature = "region")]
                if self.region.is_some() {
                    return;
                }
                if hotkey_event.id == self.keys.active_screen.id() {
                    self.handle_active_screen();
                } else if hotkey_event.id == self.keys.all_screens.id() {
                    self.handle_all_screens();
                } else if hotkey_event.id == self.keys.region_primary.id() {
                    #[cfg(feature = "region")]
                    self.handle_region(event_loop, false);
                } else if hotkey_event.id == self.keys.region_secondary.id() {
                    #[cfg(feature = "region")]
                    self.handle_region(event_loop, true);
                } else if hotkey_event.id == self.keys.open_settings.id() {
                    self.open_settings(event_loop);
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        #[cfg(feature = "region")]
        if self.region.as_ref().is_some_and(|r| r.contains(window_id)) {
            let outcome = {
                let region = self.region.as_mut().unwrap();
                match &event {
                    WindowEvent::CloseRequested => Some(SessionOutcome::Cancelled),
                    WindowEvent::RedrawRequested => {
                        region.paint(window_id);
                        None
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        region.on_cursor_moved(window_id, *position);
                        None
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        region.on_mouse_input(window_id, *button, *state)
                    }
                    WindowEvent::ModifiersChanged(mods) => {
                        region.on_modifiers(mods.state());
                        None
                    }
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        if key_event.state == ElementState::Pressed {
                            region.on_keyboard(
                                &key_event.logical_key,
                                key_event.state,
                                key_event.text.as_ref().map(|s| s.as_str()),
                            )
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            };
            if let Some(outcome) = outcome {
                self.finish_region(event_loop, outcome);
            }
            return;
        }

        #[cfg(feature = "editor")]
        if self.editor.as_ref().is_some_and(|e| e.window_id() == window_id) {
            match &event {
                WindowEvent::CloseRequested => {
                    if let Some(mut ed) = self.editor.take() {
                        ed.destroy();
                    }
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
                WindowEvent::RedrawRequested => {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.paint();
                        if ed.take_close_request() {
                            if let Some(mut ed) = self.editor.take() {
                                ed.destroy();
                            }
                        }
                    }
                }
                _ => {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.on_window_event(&event);
                    }
                }
            }
            return;
        }

        // Settings window
        match &event {
            WindowEvent::CloseRequested => {
                self.close_settings();
                event_loop.set_control_flow(ControlFlow::Wait);
                return;
            }
            WindowEvent::Destroyed => return,
            WindowEvent::RedrawRequested => {
                self.paint_settings(event_loop);
                return;
            }
            WindowEvent::Resized(size) => {
                if let Some(gl) = self.settings_gl.as_ref() {
                    gl.resize(*size);
                }
            }
            _ => {}
        }

        let Some(egui_glow) = self.settings_egui.as_mut() else {
            return;
        };
        let Some(gl_window) = self.settings_gl.as_ref() else {
            return;
        };
        let response = egui_glow.on_window_event(gl_window.window(), &event);
        if response.repaint {
            gl_window.window().request_redraw();
        }
    }
}
