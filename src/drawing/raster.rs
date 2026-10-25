//! Rasterize shapes onto softbuffer (xRGB) or BGRA buffers.

use super::shape::{Point, Shape};
use super::{COLOR_RED_BGRA, COLOR_RED_XRGB};

pub const fn stroke_width() -> i32 {
    3
}

pub fn draw_shape_xrgb(buf: &mut [u32], width: u32, height: u32, shape: &Shape) {
    match shape {
        Shape::Rect { a, b } => stroke_rect(buf, width, height, *a, *b, COLOR_RED_XRGB),
        Shape::Line { a, b } => stroke_line(buf, width, height, *a, *b, COLOR_RED_XRGB),
        Shape::Arrow { a, b } => {
            stroke_line(buf, width, height, *a, *b, COLOR_RED_XRGB);
            stroke_arrow_head(buf, width, height, *a, *b, COLOR_RED_XRGB);
        }
        Shape::Circle { center, edge } => {
            let dx = (edge.x - center.x) as f64;
            let dy = (edge.y - center.y) as f64;
            let r = (dx * dx + dy * dy).sqrt().round() as i32;
            stroke_circle(buf, width, height, *center, r, COLOR_RED_XRGB);
        }
        Shape::Freehand { points } => {
            for w in points.windows(2) {
                stroke_line(buf, width, height, w[0], w[1], COLOR_RED_XRGB);
            }
        }
        Shape::Text { at, content } => {
            stroke_text_placeholder(buf, width, height, *at, content, COLOR_RED_XRGB);
        }
    }
}

/// Composite committed (+ optional draft) shapes onto a BGRA crop (editor / Ctrl+C).
pub fn composite_shapes_bgra(base: &mut crate::capture::BgraImage, shapes: &[Shape]) {
    let w = base.width;
    let h = base.height;
    let mut xrgb = vec![0u32; (w * h) as usize];
    // Convert existing pixels so we can reuse xrgb stroke helpers, then write back.
    for (i, px) in base.bytes.chunks_exact(4).enumerate() {
        xrgb[i] = (u32::from(px[2]) << 16) | (u32::from(px[1]) << 8) | u32::from(px[0]);
    }
    for shape in shapes {
        draw_shape_xrgb(&mut xrgb, w, h, shape);
    }
    for (i, color) in xrgb.iter().enumerate() {
        let o = i * 4;
        base.bytes[o] = (*color & 0xFF) as u8;
        base.bytes[o + 1] = ((*color >> 8) & 0xFF) as u8;
        base.bytes[o + 2] = ((*color >> 16) & 0xFF) as u8;
        base.bytes[o + 3] = 0xFF;
    }
}

fn put(buf: &mut [u32], width: u32, height: u32, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let i = (y as u32 * width + x as u32) as usize;
    if i < buf.len() {
        buf[i] = color;
    }
}

fn thick(buf: &mut [u32], width: u32, height: u32, x: i32, y: i32, color: u32) {
    let t = stroke_width() / 2;
    for dy in -t..=t {
        for dx in -t..=t {
            put(buf, width, height, x + dx, y + dy, color);
        }
    }
}

fn stroke_line(buf: &mut [u32], width: u32, height: u32, a: Point, b: Point, color: u32) {
    let mut x0 = a.x;
    let mut y0 = a.y;
    let x1 = b.x;
    let y1 = b.y;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        thick(buf, width, height, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn stroke_rect(buf: &mut [u32], width: u32, height: u32, a: Point, b: Point, color: u32) {
    let left = a.x.min(b.x);
    let right = a.x.max(b.x);
    let top = a.y.min(b.y);
    let bottom = a.y.max(b.y);
    for x in left..=right {
        thick(buf, width, height, x, top, color);
        thick(buf, width, height, x, bottom, color);
    }
    for y in top..=bottom {
        thick(buf, width, height, left, y, color);
        thick(buf, width, height, right, y, color);
    }
}

fn stroke_circle(buf: &mut [u32], width: u32, height: u32, c: Point, r: i32, color: u32) {
    if r <= 0 {
        return;
    }
    let mut x = r;
    let mut y = 0;
    let mut err = 0;
    while x >= y {
        plot8(buf, width, height, c, x, y, color);
        y += 1;
        err += 1 + 2 * y;
        if 2 * (err - x) + 1 > 0 {
            x -= 1;
            err += 1 - 2 * x;
        }
    }
}

fn plot8(buf: &mut [u32], width: u32, height: u32, c: Point, x: i32, y: i32, color: u32) {
    thick(buf, width, height, c.x + x, c.y + y, color);
    thick(buf, width, height, c.x + y, c.y + x, color);
    thick(buf, width, height, c.x - y, c.y + x, color);
    thick(buf, width, height, c.x - x, c.y + y, color);
    thick(buf, width, height, c.x - x, c.y - y, color);
    thick(buf, width, height, c.x - y, c.y - x, color);
    thick(buf, width, height, c.x + y, c.y - x, color);
    thick(buf, width, height, c.x + x, c.y - y, color);
}

fn stroke_arrow_head(buf: &mut [u32], width: u32, height: u32, a: Point, b: Point, color: u32) {
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let head = 14.0;
    let wing = 7.0;
    let bx = b.x as f64 - ux * head;
    let by = b.y as f64 - uy * head;
    let lx = bx - uy * wing;
    let ly = by + ux * wing;
    let rx = bx + uy * wing;
    let ry = by - ux * wing;
    stroke_line(
        buf,
        width,
        height,
        b,
        Point::new(lx.round() as i32, ly.round() as i32),
        color,
    );
    stroke_line(
        buf,
        width,
        height,
        b,
        Point::new(rx.round() as i32, ry.round() as i32),
        color,
    );
}

/// Minimal 5×7 bitmap font for a few ASCII glyphs (Phase-2 text).
fn stroke_text_placeholder(
    buf: &mut [u32],
    width: u32,
    height: u32,
    at: Point,
    content: &str,
    color: u32,
) {
    let mut cx = at.x;
    let cy = at.y;
    for ch in content.chars().take(64) {
        if ch == '\n' {
            cx = at.x;
            // advance roughly one glyph height
            // (kept inline to avoid extra state)
            continue;
        }
        draw_glyph(buf, width, height, cx, cy, ch, color);
        cx += 8;
    }
    let _ = cy;
}

fn draw_glyph(buf: &mut [u32], width: u32, height: u32, x: i32, y: i32, ch: char, color: u32) {
    // Very small subset: letters/digits via a filled block if unknown.
    let rows: [u8; 7] = glyph_rows(ch);
    for (row, bits) in rows.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                put(buf, width, height, x + col, y + row as i32, color);
            }
        }
    }
}

fn glyph_rows(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        ' ' => [0; 7],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111],
    }
}

// Silence unused constant for now (BGRA path may use it later).
#[allow(dead_code)]
fn _red_bgra() -> [u8; 4] {
    COLOR_RED_BGRA
}
