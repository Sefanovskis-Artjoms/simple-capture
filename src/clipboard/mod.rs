//! Fast Windows clipboard image write (CF_DIBV5 from top-down BGRA).

use crate::capture::BgraImage;

/// Copy a top-down BGRA image to the clipboard (no PNG, no R↔B swap).
pub fn copy_bgra_to_clipboard(image: BgraImage) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::set_dibv5(image)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = image;
        Err("Clipboard image is only implemented on Windows for now".to_owned())
    }
}

#[cfg(target_os = "windows")]
mod win {
    use std::mem::size_of;

    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::Graphics::Gdi::{BITMAPV5HEADER, BI_BITFIELDS, LCS_GM_IMAGES};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_DIBV5;

    use crate::capture::BgraImage;

    #[allow(non_upper_case_globals)]
    const LCS_sRGB: u32 = 0x7352_4742;

    pub(super) fn set_dibv5(image: BgraImage) -> Result<(), String> {
        let width = image.width as usize;
        let height = image.height as usize;
        let row_bytes = width * 4;
        if image.bytes.len() != row_bytes * height {
            return Err("Unexpected image buffer size".to_owned());
        }

        let mut dib_pixels = vec![0u8; image.bytes.len()];
        for y in 0..height {
            let src = (height - 1 - y) * row_bytes;
            let dst = y * row_bytes;
            dib_pixels[dst..dst + row_bytes]
                .copy_from_slice(&image.bytes[src..src + row_bytes]);
        }

        let header_size = size_of::<BITMAPV5HEADER>();
        let header = BITMAPV5HEADER {
            bV5Size: header_size as u32,
            bV5Width: width as i32,
            bV5Height: height as i32,
            bV5Planes: 1,
            bV5BitCount: 32,
            bV5Compression: BI_BITFIELDS,
            bV5SizeImage: dib_pixels.len() as u32,
            bV5XPelsPerMeter: 0,
            bV5YPelsPerMeter: 0,
            bV5ClrUsed: 0,
            bV5ClrImportant: 0,
            bV5RedMask: 0x00ff_0000,
            bV5GreenMask: 0x0000_ff00,
            bV5BlueMask: 0x0000_00ff,
            bV5AlphaMask: 0xff00_0000,
            bV5CSType: LCS_sRGB,
            bV5Endpoints: Default::default(),
            bV5GammaRed: 0,
            bV5GammaGreen: 0,
            bV5GammaBlue: 0,
            bV5Intent: LCS_GM_IMAGES as u32,
            bV5ProfileData: 0,
            bV5ProfileSize: 0,
            bV5Reserved: 0,
        };

        let total_size = header_size + dib_pixels.len();
        unsafe {
            let hglobal = GlobalAlloc(GMEM_MOVEABLE, total_size)
                .map_err(|_| "Could not allocate clipboard memory".to_owned())?;
            let ptr = GlobalLock(hglobal) as *mut u8;
            if ptr.is_null() {
                let _ = GlobalFree(Some(hglobal));
                return Err("Could not lock clipboard memory".to_owned());
            }

            std::ptr::copy_nonoverlapping(
                (&header as *const BITMAPV5HEADER).cast::<u8>(),
                ptr,
                header_size,
            );
            std::ptr::copy_nonoverlapping(
                dib_pixels.as_ptr(),
                ptr.add(header_size),
                dib_pixels.len(),
            );
            let _ = GlobalUnlock(hglobal);

            OpenClipboard(None).map_err(|_| "Could not open the clipboard".to_owned())?;
            let result = (|| {
                EmptyClipboard().map_err(|_| "Could not empty the clipboard".to_owned())?;
                SetClipboardData(CF_DIBV5.0 as u32, Some(HANDLE(hglobal.0)))
                    .map_err(|_| "Could not set clipboard image data".to_owned())?;
                Ok(())
            })();
            let _ = CloseClipboard();

            if result.is_err() {
                let _ = GlobalFree(Some(HGLOBAL(hglobal.0)));
            }
            result
        }
    }
}
