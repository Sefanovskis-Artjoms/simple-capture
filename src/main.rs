//! simple-capture — modular screenshot tool (Windows-first).
//!
//! Cargo features plug/unplug subsystems — see MODULES.md.

mod app;
mod capture;
mod clipboard;
mod config;
mod gl_window;
mod hotkeys;
mod settings;
mod tray;

#[cfg(feature = "flash")]
mod flash;

#[cfg(feature = "region")]
mod region;

#[cfg(feature = "drawing")]
mod drawing;

#[cfg(feature = "editor")]
mod editor;

use app::{App, UserEvent};
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    let proxy = event_loop.create_proxy();

    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).expect("event loop error");
}
