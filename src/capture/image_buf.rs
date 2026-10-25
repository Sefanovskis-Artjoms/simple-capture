//! Shared image buffers and geometry helpers (pure logic, no windows).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl MonitorRect {
    #[allow(dead_code)]
    pub fn contains_point(self, px: i32, py: i32) -> bool {
        px >= self.x
            && py >= self.y
            && px < self.x + self.width as i32
            && py < self.y + self.height as i32
    }
}

/// Axis-aligned rect in virtual-desktop coordinates (may be unordered while dragging).
#[derive(Debug, Clone, Copy, Default)]
pub struct DragRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl DragRect {
    pub fn from_points(a: (i32, i32), b: (i32, i32)) -> Self {
        Self {
            x0: a.0,
            y0: a.1,
            x1: b.0,
            y1: b.1,
        }
    }

    pub fn normalized(self) -> MonitorRect {
        let left = self.x0.min(self.x1);
        let top = self.y0.min(self.y1);
        let right = self.x0.max(self.x1);
        let bottom = self.y0.max(self.y1);
        MonitorRect {
            x: left,
            y: top,
            width: (right - left).max(0) as u32,
            height: (bottom - top).max(0) as u32,
        }
    }
}

/// Top-down BGRA pixels (native GDI / clipboard channel order).
#[derive(Clone)]
pub struct BgraImage {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for BgraImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BgraImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes_len", &self.bytes.len())
            .finish()
    }
}

impl BgraImage {
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y * self.width + x) * 4) as usize;
        Some([
            self.bytes[i],
            self.bytes[i + 1],
            self.bytes[i + 2],
            self.bytes[i + 3],
        ])
    }
}

pub fn stitch_bgra(parts: &[(i32, i32, BgraImage)]) -> Result<BgraImage, String> {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for (x, y, image) in parts {
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

    let mut bytes = vec![0u8; width as usize * height as usize * 4];
    for (x, y, image) in parts {
        blit_bgra(
            &mut bytes,
            width,
            image,
            (*x - min_x) as u32,
            (*y - min_y) as u32,
        );
    }

    Ok(BgraImage {
        width,
        height,
        bytes,
    })
}

pub fn crop_bgra(
    parts: &[(i32, i32, BgraImage)],
    region: MonitorRect,
) -> Result<BgraImage, String> {
    if region.width == 0 || region.height == 0 {
        return Err("Empty region".to_owned());
    }
    let mut out = vec![0u8; region.width as usize * region.height as usize * 4];
    for (ox, oy, image) in parts {
        let inter = intersect(
            MonitorRect {
                x: *ox,
                y: *oy,
                width: image.width,
                height: image.height,
            },
            region,
        );
        let Some(inter) = inter else {
            continue;
        };
        for py in inter.y..inter.y + inter.height as i32 {
            for px in inter.x..inter.x + inter.width as i32 {
                let sx = (px - ox) as u32;
                let sy = (py - oy) as u32;
                let dx = (px - region.x) as u32;
                let dy = (py - region.y) as u32;
                if let Some(px_bgra) = image.pixel(sx, sy) {
                    let di = ((dy * region.width + dx) * 4) as usize;
                    out[di..di + 4].copy_from_slice(&px_bgra);
                }
            }
        }
    }
    Ok(BgraImage {
        width: region.width,
        height: region.height,
        bytes: out,
    })
}

fn intersect(a: MonitorRect, b: MonitorRect) -> Option<MonitorRect> {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.width as i32).min(b.x + b.width as i32);
    let bottom = (a.y + a.height as i32).min(b.y + b.height as i32);
    if right <= left || bottom <= top {
        return None;
    }
    Some(MonitorRect {
        x: left,
        y: top,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
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

/// Softbuffer pixel `0x00RRGGBB` from BGRA, optionally dimmed.
pub fn bgra_to_xrgb(image: &BgraImage, brightness_pct: u16) -> Vec<u32> {
    let mut out = Vec::with_capacity((image.width * image.height) as usize);
    for px in image.bytes.chunks_exact(4) {
        let b = dim(px[0], brightness_pct);
        let g = dim(px[1], brightness_pct);
        let r = dim(px[2], brightness_pct);
        out.push((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b));
    }
    out
}

fn dim(c: u8, brightness_pct: u16) -> u8 {
    ((u16::from(c) * brightness_pct) / 100) as u8
}

#[allow(dead_code)]
pub fn copy_undimmed_rect(
    dst: &mut [u32],
    dst_w: u32,
    src: &BgraImage,
    src_origin: (i32, i32),
    region: MonitorRect,
) {
    let left = region.x.max(src_origin.0);
    let top = region.y.max(src_origin.1);
    let right = region
        .x
        .saturating_add(region.width as i32)
        .min(src_origin.0 + src.width as i32);
    let bottom = region
        .y
        .saturating_add(region.height as i32)
        .min(src_origin.1 + src.height as i32);
    if right <= left || bottom <= top {
        return;
    }
    for py in top..bottom {
        for px in left..right {
            let sx = (px - src_origin.0) as u32;
            let sy = (py - src_origin.1) as u32;
            let dx = (px - src_origin.0) as u32;
            let dy = (py - src_origin.1) as u32;
            if let Some([b, g, r, _]) = src.pixel(sx, sy) {
                let i = (dy * dst_w + dx) as usize;
                if i < dst.len() {
                    dst[i] = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
                }
            }
        }
    }
}
