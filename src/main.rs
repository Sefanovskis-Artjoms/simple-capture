mod app;
mod capture;
mod clipboard;
mod config;
mod flash;
mod gdi_capture;
mod gl_window;

use app::{App, UserEvent};
use winit::event_loop::EventLoop;

fn main() {
    // println!("simple-capture running.");
    // println!("  Active screen (default Ctrl+Shift+F2) → clipboard.");
    // println!("  All screens  (default Ctrl+Shift+F3) → clipboard.");
    // println!("  Open settings from the tray menu or the open-settings hotkey.");

    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    let proxy = event_loop.create_proxy();

    let mut app = App::new(proxy);
    event_loop
        .run_app(&mut app)
        .expect("event loop error");
}
