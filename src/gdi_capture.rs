//! Lean GDI BitBlt capture → top-down BGRA (no RGBA round-trip).

use std::mem::size_of;

use windows::Win32::Graphics::Gdi::{
    BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, GetWindowDC, ReleaseDC, SRCCOPY,
    SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

/// Raw desktop pixels in top-down BGRA order (native GDI / clipboard channel order).
pub struct BgraImage {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// Capture a virtual-desktop rectangle via BitBlt + GetDIBits.
pub fn capture_rect(x: i32, y: i32, width: u32, height: u32) -> Result<BgraImage, String> {
    if width == 0 || height == 0 {
        return Err("Invalid capture size".to_owned());
    }
    let w = width as i32;
    let h = height as i32;
    let buffer_size = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| "Capture size overflow".to_owned())?;

    unsafe {
        let hwnd = GetDesktopWindow();
        let hdc_screen = GetWindowDC(Some(hwnd));
        if hdc_screen.is_invalid() {
            return Err("Could not get screen DC".to_owned());
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            let _ = ReleaseDC(Some(hwnd), hdc_screen);
            return Err("Could not create memory DC".to_owned());
        }

        let hbmp = CreateCompatibleBitmap(hdc_screen, w, h);
        if hbmp.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(Some(hwnd), hdc_screen);
            return Err("Could not create bitmap".to_owned());
        }

        let old = SelectObject(hdc_mem, hbmp.into());
        let blit_ok = BitBlt(hdc_mem, 0, 0, w, h, Some(hdc_screen), x, y, SRCCOPY).is_ok();
        let _ = SelectObject(hdc_mem, old);

        if !blit_ok {
            let _ = DeleteObject(hbmp.into());
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(Some(hwnd), hdc_screen);
            return Err("BitBlt failed".to_owned());
        }

        // Negative height ⇒ top-down BGRA in `buffer`.
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biSizeImage: buffer_size as u32,
                biCompression: 0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut buffer = vec![0u8; buffer_size];
        let got = GetDIBits(
            hdc_mem,
            hbmp,
            0,
            height,
            Some(buffer.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );

        let _ = DeleteObject(hbmp.into());
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(Some(hwnd), hdc_screen);

        if got == 0 {
            return Err("GetDIBits failed".to_owned());
        }

        Ok(BgraImage {
            width,
            height,
            bytes: buffer,
        })
    }
}
