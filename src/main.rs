mod app;
mod gl_window;

use app::{App, UserEvent};
use winit::event_loop::EventLoop;

fn main() {
    println!("simple-capture running.");
    println!("  Ctrl+Shift+F2 — primary (test)");
    println!("  Ctrl+Shift+F3 — secondary (test)");
    println!("  Ctrl+Shift+F4 — open settings (test)");

    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    let proxy = event_loop.create_proxy();

    let mut app = App::new(proxy);
    event_loop
        .run_app(&mut app)
        .expect("event loop error");
}
