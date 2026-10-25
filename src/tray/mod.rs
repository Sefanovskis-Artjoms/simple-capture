//! System tray icon + menu.

use tray_icon::menu::{Menu, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub fn build(open_id: &MenuId, quit_id: &MenuId) -> TrayIcon {
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
        .with_icon(load_icon())
        .build()
        .expect("failed to create tray icon")
}

fn load_icon() -> Icon {
    let bytes = include_bytes!("../../assets/icon.png");
    let image = image::load_from_memory(bytes)
        .expect("tray icon PNG is invalid")
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).expect("invalid tray icon")
}
