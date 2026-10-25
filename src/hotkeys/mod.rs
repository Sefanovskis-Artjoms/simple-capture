//! Global hotkey parse / register helpers.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::GlobalHotKeyManager;

use crate::config::HotkeyConfig;

pub struct RegisteredHotkeys {
    pub active_screen: HotKey,
    pub all_screens: HotKey,
    pub region_primary: HotKey,
    pub region_secondary: HotKey,
    pub open_settings: HotKey,
    pub texts: HotkeyConfig,
}

impl RegisteredHotkeys {
    pub fn as_array(&self) -> [HotKey; 5] {
        [
            self.active_screen,
            self.all_screens,
            self.region_primary,
            self.region_secondary,
            self.open_settings,
        ]
    }
}

pub fn register_startup(
    manager: &GlobalHotKeyManager,
    saved: &HotkeyConfig,
) -> RegisteredHotkeys {
    if let Ok(keys) = parse_all(saved) {
        if keys.iter().all(|k| manager.register(*k).is_ok()) {
            return RegisteredHotkeys {
                active_screen: keys[0],
                all_screens: keys[1],
                region_primary: keys[2],
                region_secondary: keys[3],
                open_settings: keys[4],
                texts: saved.clone(),
            };
        }
        for k in &keys {
            let _ = manager.unregister(*k);
        }
    }

    let defaults = HotkeyConfig::default();
    let active = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F2);
    let all = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F3);
    let region_p = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F5);
    let region_s = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F6);
    let settings = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F4);
    manager
        .register(active)
        .expect("failed to register default Ctrl+Shift+F2");
    manager
        .register(all)
        .expect("failed to register default Ctrl+Shift+F3");
    manager
        .register(region_p)
        .expect("failed to register default Ctrl+Shift+F5");
    manager
        .register(region_s)
        .expect("failed to register default Ctrl+Shift+F6");
    manager
        .register(settings)
        .expect("failed to register default Ctrl+Shift+F4");

    RegisteredHotkeys {
        active_screen: active,
        all_screens: all,
        region_primary: region_p,
        region_secondary: region_s,
        open_settings: settings,
        texts: defaults,
    }
}

pub fn parse_all(cfg: &HotkeyConfig) -> Result<[HotKey; 5], String> {
    let a: HotKey = cfg
        .active_screen
        .parse()
        .map_err(|_| "Given hotkey is invalid".to_owned())?;
    let b: HotKey = cfg
        .all_screens
        .parse()
        .map_err(|_| "Given hotkey is invalid".to_owned())?;
    let c: HotKey = cfg
        .region_primary
        .parse()
        .map_err(|_| "Given hotkey is invalid".to_owned())?;
    let d: HotKey = cfg
        .region_secondary
        .parse()
        .map_err(|_| "Given hotkey is invalid".to_owned())?;
    let e: HotKey = cfg
        .open_settings
        .parse()
        .map_err(|_| "Given hotkey is invalid".to_owned())?;

    let ids = [a.id(), b.id(), c.id(), d.id(), e.id()];
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            if ids[i] == ids[j] {
                return Err("Given hotkey is already taken".to_owned());
            }
        }
    }
    Ok([a, b, c, d, e])
}

pub fn probe(manager: &GlobalHotKeyManager, live: &[HotKey], text: &str) -> Option<String> {
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

pub fn apply(
    manager: &GlobalHotKeyManager,
    old: &[HotKey; 5],
    new: &[HotKey; 5],
) -> Result<(), String> {
    for owned in old {
        if !new.iter().any(|next| next.id() == owned.id()) {
            let _ = manager.unregister(*owned);
        }
    }
    for next in new {
        if old.iter().any(|owned| owned.id() == next.id()) {
            continue;
        }
        if manager.register(*next).is_err() {
            restore(manager, old, new);
            return Err("Given hotkey is already taken".to_owned());
        }
    }
    Ok(())
}

fn restore(manager: &GlobalHotKeyManager, old: &[HotKey; 5], new: &[HotKey; 5]) {
    for next in new {
        if !old.iter().any(|owned| owned.id() == next.id()) {
            let _ = manager.unregister(*next);
        }
    }
    for owned in old {
        let _ = manager.register(*owned);
    }
}
