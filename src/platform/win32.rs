use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetCursorPos, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, IsWindowVisible,
    SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

/// Extract the Win32 HWND from a winit window.
pub fn get_hwnd(window: &winit::window::Window) -> HWND {
    let handle = window.window_handle().expect("window handle unavailable");
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut core::ffi::c_void),
        _ => panic!("expected Win32 window handle"),
    }
}

/// Apply overlay window styles for a transparent desktop toy.
pub unsafe fn make_overlay(hwnd: HWND) {
    let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    log::info!("Window ex-style before: 0x{:08X}", style);

    // Remove WS_EX_LAYERED if present (winit's with_transparent used to set
    // it). Add WS_EX_NOREDIRECTIONBITMAP so DWM does not create a GDI
    // redirection surface — all rendering comes from the DirectComposition
    // visual that wgpu creates via DxgiFromVisual.
    const WS_EX_LAYERED: isize = 0x00080000;
    const WS_EX_NOREDIRECTIONBITMAP: isize = 0x00200000;

    let new_style = (style & !WS_EX_LAYERED)
        | WS_EX_NOACTIVATE.0 as isize
        | WS_EX_TOOLWINDOW.0 as isize
        | WS_EX_NOREDIRECTIONBITMAP;
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);

    log::info!("Window ex-style after:  0x{:08X}", new_style);

    // Force DWM to recalculate the window frame with the new styles.
    // Without this, DWM may use cached frame info from before our changes.
    let _ = SetWindowPos(
        hwnd,
        HWND::default(),
        0,
        0,
        0,
        0,
        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );

    // DWMWA_NCRENDERING_POLICY(2) = DWMNCRP_DISABLED(2)
    // Removes the 1px border DWM draws around all windows.
    let policy = 2u32;
    let _ = DwmSetWindowAttribute(
        hwnd,
        windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(2),
        &policy as *const u32 as *const core::ffi::c_void,
        4,
    );

    // DWMWA_WINDOW_CORNER_PREFERENCE(33) = DWMWCP_DONOTROUND(1)
    let corner = 1u32;
    let _ = DwmSetWindowAttribute(
        hwnd,
        windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(33),
        &corner as *const u32 as *const core::ffi::c_void,
        4,
    );

    // DWMWA_BORDER_COLOR(34) = DWMWA_COLOR_NONE(0xFFFFFFFE)
    let no_border = 0xFFFFFFFEu32;
    let _ = DwmSetWindowAttribute(
        hwnd,
        windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(34),
        &no_border as *const u32 as *const core::ffi::c_void,
        4,
    );

    // DWMWA_SYSTEMBACKDROP_TYPE(38) = DWMSBT_NONE(1)
    // Disables Mica/Acrylic/glass blur behind the window so the extended
    // DWM frame is truly transparent, not frosted.
    let backdrop = 1u32;
    let _ = DwmSetWindowAttribute(
        hwnd,
        windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(38),
        &backdrop as *const u32 as *const core::ffi::c_void,
        4,
    );
}

/// Set up the window as a transparent, click-through, always-on-top overlay.
pub fn setup_overlay(window: &winit::window::Window) {
    window
        .set_cursor_hittest(false)
        .expect("failed to set cursor hittest");

    let hwnd = get_hwnd(window);
    unsafe {
        make_overlay(hwnd);
    }

    log::info!("Win32 overlay setup complete (DirectComposition + click-through + toolwindow)");
}

/// Get the current global mouse cursor position in screen pixels.
pub fn get_mouse_pos() -> (f32, f32) {
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    (point.x as f32, point.y as f32)
}

/// Check if the ESC key is currently pressed (works regardless of window focus).
pub fn is_escape_pressed() -> bool {
    // VK_ESCAPE = 0x1B. High bit set = key is currently down.
    unsafe { GetAsyncKeyState(0x1B) & (0x8000u16 as i16) != 0 }
}

/// Check if F12 was pressed since last call.
/// Uses low bit (transition) to detect single press, not held state.
pub fn is_f12_pressed() -> bool {
    // VK_F12 = 0x7B. Low bit = key was pressed since last call to GetAsyncKeyState.
    unsafe { GetAsyncKeyState(0x7B) & 1 != 0 }
}

/// Check if Y key was pressed since last call (yarn ball hotkey).
pub fn is_y_pressed() -> bool {
    // VK_Y = 0x59
    unsafe { GetAsyncKeyState(0x59) & 1 != 0 }
}

/// Check if B key was pressed since last call (cardboard box hotkey).
pub fn is_b_pressed() -> bool {
    // VK_B = 0x42
    unsafe { GetAsyncKeyState(0x42) & 1 != 0 }
}

/// Check if F11 was pressed since last call (mode cycle hotkey).
pub fn is_f11_pressed() -> bool {
    // VK_F11 = 0x7A
    unsafe { GetAsyncKeyState(0x7A) & 1 != 0 }
}

/// Get seconds since last user input (keyboard/mouse, system-wide).
/// Uses GetLastInputInfo Win32 API — one cheap syscall.
pub fn get_idle_time() -> f64 {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            let tick_count = windows::Win32::System::SystemInformation::GetTickCount();
            let elapsed_ms = tick_count.wrapping_sub(info.dwTime);
            elapsed_ms as f64 / 1000.0
        } else {
            0.0
        }
    }
}

/// Get the current local hour as a float (0.0-24.0, e.g. 14.5 = 2:30 PM).
pub fn get_local_hour() -> f32 {
    unsafe {
        let st = windows::Win32::System::SystemInformation::GetLocalTime();
        st.wHour as f32 + st.wMinute as f32 / 60.0 + st.wSecond as f32 / 3600.0
    }
}

/// Get mouse button states. Returns (left_down, right_down, middle_down).
/// All buttons check both held state (high bit) and transition bit (low bit)
/// to catch quick clicks that release between polls (e.g. right-click opening
/// a context menu on the desktop behind our click-through overlay).
pub fn get_mouse_buttons() -> (bool, bool, bool) {
    unsafe {
        let l = GetAsyncKeyState(0x01); // VK_LBUTTON
        let r = GetAsyncKeyState(0x02); // VK_RBUTTON
        let m = GetAsyncKeyState(0x04); // VK_MBUTTON
        let held = 0x8000u16 as i16;
        let left = (l & held != 0) || (l & 1 != 0);
        let right = (r & held != 0) || (r & 1 != 0);
        let middle = (m & held != 0) || (m & 1 != 0);
        (left, right, middle)
    }
}

// ---------------------------------------------------------------------------
// Window Enumeration (Task #19)
// ---------------------------------------------------------------------------

/// A rectangle representing a visible desktop window.
#[derive(Debug, Clone)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub title: String,
}

/// Enumerate all visible, non-tool windows on the desktop.
/// Excludes our own overlay and windows with zero area.
/// Call this periodically (e.g., every few seconds), NOT every frame.
pub fn enumerate_windows(own_hwnd: HWND) -> Vec<WindowRect> {
    struct EnumState {
        own_hwnd: HWND,
        results: Vec<WindowRect>,
    }

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut EnumState);

        // Skip our own window
        if hwnd == state.own_hwnd {
            return TRUE;
        }

        // Skip invisible windows
        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }

        // Skip tool windows (tooltips, floating toolbars, etc.)
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if (ex_style as u32) & WS_EX_TOOLWINDOW.0 != 0 {
            return TRUE;
        }

        // Get window rect
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return TRUE;
        }

        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;

        // Skip zero-area windows
        if w <= 0 || h <= 0 {
            return TRUE;
        }

        // Get title (first 256 chars)
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, &mut buf);
        let title = if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        };

        state.results.push(WindowRect {
            x: rect.left,
            y: rect.top,
            w,
            h,
            title,
        });

        TRUE
    }

    let mut state = EnumState {
        own_hwnd,
        results: Vec::with_capacity(64),
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut state as *mut EnumState as isize),
        );
    }

    state.results
}
