//! Softbuffer freeze overlay with selection punch-through (+ optional drawings).

use std::num::NonZeroU32;
use std::sync::Arc;

use softbuffer::{Context, Surface};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, Window, WindowId, WindowLevel};

use crate::capture::{bgra_to_xrgb, BgraImage, MonitorRect};

#[cfg(feature = "drawing")]
use crate::drawing::{draw_shape_xrgb, Shape};

const BRIGHTNESS: u16 = 55;

struct OverlayWindow {
    window: Arc<Window>,
    surface: Surface<Arc<Window>, Arc<Window>>,
    _context: Context<Arc<Window>>,
    /// Dimmed full-monitor softbuffer pixels.
    dimmed: Vec<u32>,
    /// Undimmed source for punch-through.
    source: BgraImage,
    origin: (i32, i32),
    width: u32,
    height: u32,
}

pub struct RegionOverlay {
    windows: Vec<OverlayWindow>,
}

impl RegionOverlay {
    pub fn show(
        event_loop: &ActiveEventLoop,
        parts: &[(i32, i32, BgraImage)],
    ) -> Result<Self, String> {
        let mut windows = Vec::with_capacity(parts.len());
        for (x, y, image) in parts {
            let rect = MonitorRect {
                x: *x,
                y: *y,
                width: image.width,
                height: image.height,
            };
            if rect.width == 0 || rect.height == 0 {
                continue;
            }
            windows.push(create_window(event_loop, &rect, image)?);
        }
        if windows.is_empty() {
            return Err("No region overlay windows created".to_owned());
        }
        windows[0].window.focus_window();
        for w in &windows {
            w.window.request_redraw();
        }
        Ok(Self { windows })
    }

    pub fn contains(&self, id: WindowId) -> bool {
        self.windows.iter().any(|w| w.window.id() == id)
    }

    pub fn window_ids(&self) -> Vec<WindowId> {
        self.windows.iter().map(|w| w.window.id()).collect()
    }

    pub fn origin_of(&self, id: WindowId) -> Option<(i32, i32)> {
        self.windows
            .iter()
            .find(|w| w.window.id() == id)
            .map(|w| w.origin)
    }

    pub fn paint_id(
        &mut self,
        id: WindowId,
        _parts: &[(i32, i32, BgraImage)],
        selection: Option<MonitorRect>,
        #[cfg(feature = "drawing")] shapes: &[&Shape],
    ) {
        let Some(win) = self.windows.iter_mut().find(|w| w.window.id() == id) else {
            return;
        };
        #[cfg(feature = "drawing")]
        paint_window(win, selection, shapes);
        #[cfg(not(feature = "drawing"))]
        paint_window(win, selection);
    }
}

fn paint_window(
    win: &mut OverlayWindow,
    selection: Option<MonitorRect>,
    #[cfg(feature = "drawing")] shapes: &[&Shape],
) {
    let Some(width) = NonZeroU32::new(win.width) else {
        return;
    };
    let Some(height) = NonZeroU32::new(win.height) else {
        return;
    };
    if win.surface.resize(width, height).is_err() {
        return;
    }

    // Copy fields we need so buffer_mut() can borrow surface alone.
    let dimmed = win.dimmed.clone();
    let origin = win.origin;
    let win_w = win.width;
    let win_h = win.height;
    let source = win.source.clone();

    let Ok(mut buffer) = win.surface.buffer_mut() else {
        return;
    };
    if buffer.len() != dimmed.len() {
        return;
    }
    buffer.copy_from_slice(&dimmed);

    if let Some(sel) = selection {
        punch_through(&source, origin, win_w, &mut buffer, sel);
        stroke_selection_border(&mut buffer, win_w, win_h, origin, sel);

        #[cfg(feature = "drawing")]
        {
            for shape in shapes {
                draw_shape_on_monitor(origin, win_w, win_h, &mut buffer, sel, shape);
            }
        }
    }

    let _ = buffer.present();
}

fn punch_through(
    source: &BgraImage,
    origin: (i32, i32),
    win_w: u32,
    buffer: &mut [u32],
    sel: MonitorRect,
) {
    let left = sel.x.max(origin.0);
    let top = sel.y.max(origin.1);
    let right = (sel.x + sel.width as i32).min(origin.0 + source.width as i32);
    let bottom = (sel.y + sel.height as i32).min(origin.1 + source.height as i32);
    if right <= left || bottom <= top {
        return;
    }
    for py in top..bottom {
        for px in left..right {
            let sx = (px - origin.0) as u32;
            let sy = (py - origin.1) as u32;
            if let Some([b, g, r, _]) = source.pixel(sx, sy) {
                let i = (sy * win_w + sx) as usize;
                if i < buffer.len() {
                    buffer[i] = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
                }
            }
        }
    }
}

fn stroke_selection_border(
    buffer: &mut [u32],
    width: u32,
    height: u32,
    origin: (i32, i32),
    sel: MonitorRect,
) {
    let color = 0x00FF_FFFF;
    let left = sel.x - origin.0;
    let top = sel.y - origin.1;
    let right = left + sel.width as i32 - 1;
    let bottom = top + sel.height as i32 - 1;
    for x in left..=right {
        put(buffer, width, height, x, top, color);
        put(buffer, width, height, x, bottom, color);
    }
    for y in top..=bottom {
        put(buffer, width, height, left, y, color);
        put(buffer, width, height, right, y, color);
    }
}

fn put(buffer: &mut [u32], width: u32, height: u32, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let i = (y as u32 * width + x as u32) as usize;
    if i < buffer.len() {
        buffer[i] = color;
    }
}

#[cfg(feature = "drawing")]
fn draw_shape_on_monitor(
    origin: (i32, i32),
    win_w: u32,
    win_h: u32,
    buffer: &mut [u32],
    sel: MonitorRect,
    shape: &Shape,
) {
    let sw = sel.width;
    let sh = sel.height;
    if sw == 0 || sh == 0 {
        return;
    }
    let mut tmp = vec![0u32; (sw * sh) as usize];
    for sy in 0..sh {
        for sx in 0..sw {
            let dx = sel.x + sx as i32 - origin.0;
            let dy = sel.y + sy as i32 - origin.1;
            if dx >= 0 && dy >= 0 && dx < win_w as i32 && dy < win_h as i32 {
                let i = (dy as u32 * win_w + dx as u32) as usize;
                tmp[(sy * sw + sx) as usize] = buffer[i];
            }
        }
    }
    draw_shape_xrgb(&mut tmp, sw, sh, shape);
    for sy in 0..sh {
        for sx in 0..sw {
            let dx = sel.x + sx as i32 - origin.0;
            let dy = sel.y + sy as i32 - origin.1;
            if dx >= 0 && dy >= 0 && dx < win_w as i32 && dy < win_h as i32 {
                let i = (dy as u32 * win_w + dx as u32) as usize;
                buffer[i] = tmp[(sy * sw + sx) as usize];
            }
        }
    }
}

fn create_window(
    event_loop: &ActiveEventLoop,
    rect: &MonitorRect,
    image: &BgraImage,
) -> Result<OverlayWindow, String> {
    let mut attrs = Window::default_attributes()
        .with_title("simple-capture region")
        .with_decorations(false)
        .with_resizable(false)
        .with_transparent(false)
        .with_active(true)
        .with_cursor(CursorIcon::Crosshair)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_position(PhysicalPosition::new(rect.x, rect.y))
        .with_inner_size(PhysicalSize::new(rect.width, rect.height));

    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs = attrs.with_skip_taskbar(true);
    }

    let window = event_loop
        .create_window(attrs)
        .map_err(|_| "Could not create region overlay".to_owned())?;
    let window = Arc::new(window);

    let context = Context::new(window.clone())
        .map_err(|_| "Could not create overlay draw context".to_owned())?;
    let surface = Surface::new(&context, window.clone())
        .map_err(|_| "Could not create overlay surface".to_owned())?;

    let dimmed = bgra_to_xrgb(image, BRIGHTNESS);

    Ok(OverlayWindow {
        window,
        surface,
        _context: context,
        dimmed,
        source: image.clone(),
        origin: (rect.x, rect.y),
        width: image.width,
        height: image.height,
    })
}
