//! Screen capture → flash ASAP → clipboard later.
//!
//! Capture uses lean GDI BitBlt into BGRA. Stitching (all-screens) and clipboard
//! encoding happen after the flash is shown.

use std::time::Instant;

use xcap::Monitor;

use crate::clipboard;
use crate::gdi_capture::{self, BgraImage};

#[derive(Debug, Clone, Copy)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Pixels ready for flash; stitch/clipboard may still be pending.
pub struct CaptureResult {
    pub rects: Vec<MonitorRect>,
    /// Virtual-desktop origin + image for each monitor (unstitched).
    pub parts: Vec<(i32, i32, BgraImage)>,
    /// Time until pixels were in RAM (excludes stitch/clipboard).
    #[allow(dead_code)] // used by commented timing logs
    pub capture_ms: u128,
}

/// Prime GDI + monitor enumeration.
pub fn warm_up() {
    let Ok(monitors) = Monitor::all() else {
        return;
    };
    for monitor in monitors.iter().take(2) {
        if let Ok(rect) = monitor_rect(monitor) {
            let _ = gdi_capture::capture_rect(rect.x, rect.y, rect.width, rect.height);
        }
    }
}

pub fn capture_active_monitor() -> Result<CaptureResult, String> {
    let started = Instant::now();
    let (x, y) = cursor_position()?;
    let monitor =
        Monitor::from_point(x, y).map_err(|_| "Could not find the active monitor".to_owned())?;
    let rect = monitor_rect(&monitor)?;
    let image = gdi_capture::capture_rect(rect.x, rect.y, rect.width, rect.height)?;
    Ok(CaptureResult {
        rects: vec![rect],
        parts: vec![(rect.x, rect.y, image)],
        capture_ms: started.elapsed().as_millis(),
    })
}

/// Capture every monitor in parallel. Does **not** stitch — call
/// [`finish_clipboard_image`] on a background thread after flashing.
pub fn capture_all_monitors() -> Result<CaptureResult, String> {
    let started = Instant::now();
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
            let image = gdi_capture::capture_rect(rect.x, rect.y, rect.width, rect.height)?;
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

    Ok(CaptureResult {
        rects,
        parts,
        capture_ms: started.elapsed().as_millis(),
    })
}

/// Stitch if needed, then write clipboard. Run off the UI thread.
pub fn finish_clipboard_image(parts: Vec<(i32, i32, BgraImage)>) -> Result<(), String> {
    let image = if parts.len() == 1 {
        parts.into_iter().next().unwrap().2
    } else {
        stitch_bgra(parts)?
    };
    clipboard::copy_bgra_to_clipboard(image)
}

fn stitch_bgra(parts: Vec<(i32, i32, BgraImage)>) -> Result<BgraImage, String> {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for (x, y, image) in &parts {
        min_x = min_x.min(*x);
        min_y = min_y.min(*y);
        max_x = max_x.max(*x + image.width as i32);
        max_y = max_y.max(*y + image.height as i32);
    }

    let width = u32::try_from(max_x - min_x).map_err(|_| "Invalid desktop width".to_owned())?;
    let height = u32::try_from(max_y - min_y).map_err(|_| "Invalid desktop height".to_owned())?;
    if width == 0 || height == 0 {
        return Err("Invalid desktop size".to_owned());
    }

    let mut bytes = vec![0u8; (width as usize) * (height as usize) * 4];
    for (x, y, image) in &parts {
        let dx = (x - min_x) as u32;
        let dy = (y - min_y) as u32;
        blit_bgra(&mut bytes, width, image, dx, dy);
    }

    Ok(BgraImage {
        width,
        height,
        bytes,
    })
}

fn blit_bgra(dst: &mut [u8], dst_width: u32, src: &BgraImage, dx: u32, dy: u32) {
    let dst_stride = dst_width as usize * 4;
    let src_stride = src.width as usize * 4;
    for row in 0..src.height as usize {
        let dst_row = (dy as usize + row) * dst_stride + dx as usize * 4;
        let src_row = row * src_stride;
        dst[dst_row..dst_row + src_stride]
            .copy_from_slice(&src.bytes[src_row..src_row + src_stride]);
    }
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
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok == 0 {
        return Err("Could not get cursor position".to_owned());
    }
    Ok((point.x, point.y))
}

#[cfg(not(target_os = "windows"))]
fn cursor_position() -> Result<(i32, i32), String> {
    Ok((0, 0))
}
