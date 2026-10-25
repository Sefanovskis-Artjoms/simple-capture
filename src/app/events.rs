//! Cross-thread events into the winit loop.

use std::time::Duration;

use global_hotkey::GlobalHotKeyEvent;
use tray_icon::menu::MenuEvent;

#[derive(Debug)]
pub enum UserEvent {
    Menu(MenuEvent),
    Hotkey(GlobalHotKeyEvent),
    Redraw(Duration),
    EndFlash,
}
