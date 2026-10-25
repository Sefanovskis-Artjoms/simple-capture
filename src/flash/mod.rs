//! Brief white flash after a successful simple capture.

use std::time::Duration;

use crate::capture::MonitorRect;

const FLASH_ALPHA: u8 = 16;
pub const FLASH_DURATION: Duration = Duration::from_millis(200);

pub struct FlashOverlay {
    #[cfg(target_os = "windows")]
    hwnds: Vec<windows::Win32::Foundation::HWND>,
}

impl FlashOverlay {
    pub fn show(rects: &[MonitorRect]) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        {
            win::show(rects)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = rects;
            Ok(Self {})
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for FlashOverlay {
    fn drop(&mut self) {
        for hwnd in self.hwnds.drain(..) {
            win::destroy(hwnd);
        }
    }
}

#[cfg(target_os = "windows")]
mod win {
    use std::sync::Once;

    use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        GetStockObject, RedrawWindow, UpdateWindow, WHITE_BRUSH, HBRUSH, RDW_ERASE,
        RDW_INVALIDATE, RDW_UPDATENOW,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, SetLayeredWindowAttributes,
        ShowWindow, CS_HREDRAW, CS_VREDRAW, LWA_ALPHA, SW_SHOWNOACTIVATE, WM_NCHITTEST,
        WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_EX_TRANSPARENT, WS_POPUP, HTTRANSPARENT,
    };
    use windows::core::w;

    use super::{FlashOverlay, FLASH_ALPHA};
    use crate::capture::MonitorRect;

    const CLASS_NAME: windows::core::PCWSTR = w!("simple_capture_flash");
    static REGISTER_CLASS: Once = Once::new();

    pub fn show(rects: &[MonitorRect]) -> Result<FlashOverlay, String> {
        register_class()?;
        let mut hwnds = Vec::with_capacity(rects.len());
        for rect in rects {
            if rect.width == 0 || rect.height == 0 {
                continue;
            }
            match create_hwnd(rect) {
                Ok(hwnd) => hwnds.push(hwnd),
                Err(err) => {
                    for hwnd in hwnds.drain(..) {
                        destroy(hwnd);
                    }
                    return Err(err);
                }
            }
        }
        if hwnds.is_empty() {
            return Err("No flash windows created".to_owned());
        }
        Ok(FlashOverlay { hwnds })
    }

    pub fn destroy(hwnd: HWND) {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }

    fn register_class() -> Result<(), String> {
        let mut result = Ok(());
        REGISTER_CLASS.call_once(|| {
            let instance = match unsafe { GetModuleHandleW(None) } {
                Ok(module) => HINSTANCE(module.0),
                Err(_) => {
                    result = Err("Could not get module handle".to_owned());
                    return;
                }
            };

            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(flash_wnd_proc),
                hInstance: instance,
                hbrBackground: HBRUSH(unsafe { GetStockObject(WHITE_BRUSH) }.0),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };

            if unsafe { RegisterClassExW(&class) } == 0 {
                result = Err("Could not register flash window class".to_owned());
            }
        });
        result
    }

    fn create_hwnd(rect: &MonitorRect) -> Result<HWND, String> {
        let instance = unsafe { GetModuleHandleW(None) }
            .map_err(|_| "Could not get module handle".to_owned())?;

        let ex_style = WS_EX_LAYERED
            | WS_EX_TRANSPARENT
            | WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE;

        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                CLASS_NAME,
                w!(""),
                WS_POPUP,
                rect.x,
                rect.y,
                rect.width as i32,
                rect.height as i32,
                None,
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )
        }
        .map_err(|_| "Could not create flash overlay".to_owned())?;

        unsafe {
            SetLayeredWindowAttributes(hwnd, COLORREF(0), FLASH_ALPHA, LWA_ALPHA)
                .map_err(|_| "Could not set flash opacity".to_owned())?;
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = UpdateWindow(hwnd);
            let _ = RedrawWindow(
                Some(hwnd),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
            );
        }

        Ok(hwnd)
    }

    unsafe extern "system" fn flash_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_NCHITTEST {
            return LRESULT(HTTRANSPARENT as isize);
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}
