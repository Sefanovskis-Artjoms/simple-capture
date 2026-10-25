//! Screen capture public API.

mod gdi;
pub mod image_buf;

pub use image_buf::{
    bgra_to_xrgb, crop_bgra, stitch_bgra, BgraImage, DragRect, MonitorRect,
};

use xcap::Monitor;

pub struct CaptureResult {
    pub rects: Vec<MonitorRect>,
    pub parts: Vec<(i32, i32, BgraImage)>,
}

pub fn warm_up() {
    let Ok(monitors) = Monitor::all() else {
        return;
    };
    for monitor in monitors.iter().take(2) {
        if let Ok(rect) = monitor_rect(monitor) {
            let _ = gdi::capture_rect(rect.x, rect.y, rect.width, rect.height);
        }
    }
}

pub fn capture_active_monitor() -> Result<CaptureResult, String> {
    let (x, y) = cursor_position()?;
    let monitor =
        Monitor::from_point(x, y).map_err(|_| "Could not find the active monitor".to_owned())?;
    let rect = monitor_rect(&monitor)?;
    let image = gdi::capture_rect(rect.x, rect.y, rect.width, rect.height)?;
    Ok(CaptureResult {
        rects: vec![rect],
        parts: vec![(rect.x, rect.y, image)],
    })
}

pub fn capture_all_monitors() -> Result<CaptureResult, String> {
    let monitors = Monitor::all().map_err(|_| "Could not list monitors".to_owned())?;
    if monitors.is_empty() {
        return Err("No monitors found".to_owned());
    }

    let mut rects = Vec::with_capacity(monitors.len());
    for monitor in &monitors {
        rects.push(monitor_rect(monitor)?);
    }

    let mut handles = Vec::with_capacity(rects.len());
    for rect in rects.iter().copied() {
        handles.push(std::thread::spawn(move || {
            let image = gdi::capture_rect(rect.x, rect.y, rect.width, rect.height)?;
            Ok::<_, String>((rect.x, rect.y, image))
        }));
    }

    let mut parts = Vec::with_capacity(handles.len());
    for handle in handles {
        parts.push(
            handle
                .join()
                .map_err(|_| "Capture thread panicked".to_owned())??,
        );
    }

    Ok(CaptureResult { rects, parts })
}

pub fn finish_clipboard_image(parts: Vec<(i32, i32, BgraImage)>) -> Result<(), String> {
    let image = if parts.len() == 1 {
        parts.into_iter().next().unwrap().2
    } else {
        stitch_bgra(&parts)?
    };
    crate::clipboard::copy_bgra_to_clipboard(image)
}

fn monitor_rect(monitor: &Monitor) -> Result<MonitorRect, String> {
    Ok(MonitorRect {
        x: monitor
            .x()
            .map_err(|_| "Could not read monitor position".to_owned())?,
        y: monitor
            .y()
            .map_err(|_| "Could not read monitor position".to_owned())?,
        width: monitor
            .width()
            .map_err(|_| "Could not read monitor size".to_owned())?,
        height: monitor
            .height()
            .map_err(|_| "Could not read monitor size".to_owned())?,
    })
}

#[cfg(target_os = "windows")]
fn cursor_position() -> Result<(i32, i32), String> {
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    unsafe extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
    }
    let mut point = Point { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err("Could not get cursor position".to_owned());
    }
    Ok((point.x, point.y))
}

#[cfg(not(target_os = "windows"))]
fn cursor_position() -> Result<(i32, i32), String> {
    Ok((0, 0))
}
