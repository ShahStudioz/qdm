//! Native platform window customization for borderless resizing and window management.

#[cfg(target_os = "windows")]
pub mod windows {
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::UI::Shell::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    static IS_MAXIMIZED: AtomicBool = AtomicBool::new(false);

    const SUBCLASS_ID: usize = 4289;
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_DONOTROUND: u32 = 1;
    const DWMWCP_ROUND: u32 = 2;

    #[repr(C)]
    struct Margins {
        cx_left_width: i32,
        cx_right_width: i32,
        cy_top_height: i32,
        cy_bottom_height: i32,
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: HWND,
            dwattribute: u32,
            pvattribute: *const std::ffi::c_void,
            cbattribute: u32,
        ) -> i32;
        fn DwmExtendFrameIntoClientArea(hwnd: HWND, pmarinset: *const Margins) -> i32;
    }

    unsafe fn apply_dwm_corner_preference(hwnd: HWND, maximized: bool) {
        let preference: u32 = if maximized {
            DWMWCP_DONOTROUND
        } else {
            DWMWCP_ROUND
        };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const u32 as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }

    /// Returns whether the window is currently maximized.
    pub fn is_window_maximized() -> bool {
        IS_MAXIMIZED.load(Ordering::Relaxed)
    }

    /// Sets the maximize tracking state.
    pub fn set_window_maximized(max: bool) {
        IS_MAXIMIZED.store(max, Ordering::Relaxed);
    }

    /// Configures the Win32 window to allow border grabbing and resizing
    /// while remaining borderless, frameless, and rounded on Windows 11.
    pub unsafe fn init_borderless_resize(hwnd: isize) {
        if INITIALIZED.swap(true, Ordering::SeqCst) {
            return;
        }

        let hwnd = hwnd as HWND;
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;

        // Restore WS_THICKFRAME to signal to Windows that this window is resizable
        SetWindowLongW(hwnd, GWL_STYLE, (style | WS_THICKFRAME) as i32);
        SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );

        // Extend DWM sheet into client area so transparent rounded corners have no white backing
        let margins = Margins {
            cx_left_width: -1,
            cx_right_width: -1,
            cy_top_height: -1,
            cy_bottom_height: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

        // Enable native Windows 11 DWM rounded corners when windowed
        apply_dwm_corner_preference(hwnd, IsZoomed(hwnd) != 0);

        // Subclass using standard safe Win32 Shell API
        SetWindowSubclass(hwnd, Some(subclass_wndproc), SUBCLASS_ID, 0);
    }

    unsafe extern "system" fn subclass_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _uid_subclass: usize,
        _dw_ref_data: usize,
    ) -> LRESULT {
        match msg {
            // 1. Prevent background erase flashes/vibration during resizing
            WM_ERASEBKGND => {
                return 1;
            }

            // 2. Prevent Windows from BitBlt-ing stale bits when moving/resizing edges
            WM_WINDOWPOSCHANGING => {
                let pos = lparam as *mut WINDOWPOS;
                if !pos.is_null() {
                    (*pos).flags |= SWP_NOCOPYBITS;
                }
            }

            // 3. Track maximize/restore state synchronously and update DWM corner rounding
            WM_SIZE => {
                if wparam == SIZE_MAXIMIZED as usize {
                    IS_MAXIMIZED.store(true, Ordering::Relaxed);
                    apply_dwm_corner_preference(hwnd, true);
                } else if wparam == SIZE_RESTORED as usize {
                    IS_MAXIMIZED.store(false, Ordering::Relaxed);
                    apply_dwm_corner_preference(hwnd, false);
                }
            }

            // 4. Client area calculation for borderless window
            WM_NCCALCSIZE => {
                if wparam != 0 && IsZoomed(hwnd) == 0 {
                    // Return 0 so client area fills the entire window rectangle
                    return 0;
                }
            }

            // 5. Border and corner hit testing for edge-grabbing
            WM_NCHITTEST if IsZoomed(hwnd) == 0 => {
                let mut rect = std::mem::zeroed();
                if GetWindowRect(hwnd, &mut rect) != 0 {
                    let x = (lparam & 0xFFFF) as i16 as i32;
                    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                    let border = 7;
                    let corner = 14;

                    let on_left = x >= rect.left && x < rect.left + border;
                    let on_right = x <= rect.right && x > rect.right - border;
                    let on_top = y >= rect.top && y < rect.top + border;
                    let on_bottom = y <= rect.bottom && y > rect.bottom - border;

                    let corner_left = x >= rect.left && x < rect.left + corner;
                    let corner_right = x <= rect.right && x > rect.right - corner;
                    let corner_top = y >= rect.top && y < rect.top + corner;
                    let corner_bottom = y <= rect.bottom && y > rect.bottom - corner;

                    if corner_top && corner_left {
                        return HTTOPLEFT as LRESULT;
                    }
                    if corner_top && corner_right {
                        return HTTOPRIGHT as LRESULT;
                    }
                    if corner_bottom && corner_left {
                        return HTBOTTOMLEFT as LRESULT;
                    }
                    if corner_bottom && corner_right {
                        return HTBOTTOMRIGHT as LRESULT;
                    }
                    if on_left {
                        return HTLEFT as LRESULT;
                    }
                    if on_right {
                        return HTRIGHT as LRESULT;
                    }
                    if on_top {
                        return HTTOP as LRESULT;
                    }
                    if on_bottom {
                        return HTBOTTOM as LRESULT;
                    }
                }
            }

            _ => {}
        }

        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

#[cfg(not(target_os = "windows"))]
pub mod windows {
    pub fn is_window_maximized() -> bool {
        false
    }
    pub fn set_window_maximized(_max: bool) {}
}
