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

use crate::gl_window::GlutinWindowContext;

#[derive(Debug)]
pub enum UserEvent {
    Menu(MenuEvent),
    Hotkey(GlobalHotKeyEvent),
    Redraw(Duration),
}

pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    _hotkey_manager: GlobalHotKeyManager,
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
        let primary = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F2);
        let secondary = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F3);
        let open_settings = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F4);
        hotkey_manager
            .register(primary)
            .expect("failed to register Ctrl+Shift+F2");
        hotkey_manager
            .register(secondary)
            .expect("failed to register Ctrl+Shift+F3");
        hotkey_manager
            .register(open_settings)
            .expect("failed to register Ctrl+Shift+F4");

        Self {
            proxy,
            _hotkey_manager: hotkey_manager,
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
            primary_text: "Ctrl+Shift+F2".to_owned(),
            secondary_text: "Ctrl+Shift+F3".to_owned(),
            open_settings_text: "Ctrl+Shift+F4".to_owned(),
        }
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

                ui.horizontal(|ui| {
                    ui.label("Primary capture:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.primary_text).desired_width(220.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Secondary capture:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.secondary_text).desired_width(220.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Open settings:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.open_settings_text)
                            .desired_width(220.0),
                    );
                });

                ui.add_space(12.0);
                if ui.button("Save").clicked() {
                    println!("[settings] primary: {}", self.primary_text);
                    println!("[settings] secondary: {}", self.secondary_text);
                    println!("[settings] open settings: {}", self.open_settings_text);
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
            UserEvent::Menu(menu_event) => {
                if menu_event.id == self.open_id {
                    self.open_settings_window(event_loop);
                } else if menu_event.id == self.quit_id {
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
                    println!("[primary] Ctrl+Shift+F2");
                } else if hotkey_event.id == self.secondary.id() {
                    println!("[secondary] Ctrl+Shift+F3");
                } else if hotkey_event.id == self.open_settings.id() {
                    println!("[settings] Ctrl+Shift+F4");
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
