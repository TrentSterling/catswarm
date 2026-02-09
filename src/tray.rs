/// System tray icon with right-click context menu.
/// Uses Win32 Shell_NotifyIconW API directly — no extra crate needed.

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, LoadIconW, PostMessageW, RegisterClassW, SetForegroundWindow, TrackPopupMenu,
    CS_HREDRAW, CS_VREDRAW, HMENU, IDI_APPLICATION, MF_SEPARATOR, MF_STRING, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, WM_COMMAND, WM_DESTROY, WM_USER, WNDCLASSW, WS_EX_TOOLWINDOW,
};

/// Custom message ID for tray icon callbacks.
const WM_TRAYICON: u32 = WM_USER + 1;

/// Menu item IDs.
const ID_QUIT: u16 = 1000;
const ID_MODE_WORK: u16 = 1001;
const ID_MODE_PLAY: u16 = 1002;
const ID_MODE_ZEN: u16 = 1003;
const ID_MODE_CHAOS: u16 = 1004;
const ID_PAUSE: u16 = 1005;
const ID_DEBUG: u16 = 1006;

/// Commands returned from tray menu interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    None,
    Quit,
    SetModeWork,
    SetModePlay,
    SetModeZen,
    SetModeChaos,
    TogglePause,
    ToggleDebug,
}

/// System tray icon state.
pub struct TrayIcon {
    #[cfg(windows)]
    hwnd: HWND,
    #[cfg(windows)]
    nid: NOTIFYICONDATAW,
    /// Pending command from the last menu interaction.
    pub pending_command: TrayCommand,
}

#[cfg(windows)]
impl TrayIcon {
    pub fn new() -> Self {
        unsafe {
            // Register a hidden window class for receiving tray messages.
            let class_name: Vec<u16> = "PetToyTrayClass\0".encode_utf16().collect();
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(tray_wnd_proc),
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wc);

            // Create a hidden message-only window.
            use windows::Win32::Foundation::HINSTANCE;
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::PCWSTR::null(),
                Default::default(),
                0,
                0,
                0,
                0,
                HWND::default(),
                HMENU::default(),
                HINSTANCE::default(),
                None,
            )
            .expect("failed to create tray message window");

            // Build NOTIFYICONDATAW
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;

            // Use default application icon
            nid.hIcon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();

            // Tooltip text
            let tip = "PetToy - Desktop Cats";
            for (i, ch) in tip.encode_utf16().enumerate() {
                if i >= nid.szTip.len() - 1 {
                    break;
                }
                nid.szTip[i] = ch;
            }

            let _ = Shell_NotifyIconW(NIM_ADD, &nid);

            log::info!("System tray icon created");

            Self {
                hwnd,
                nid,
                pending_command: TrayCommand::None,
            }
        }
    }

    /// Poll for tray menu commands. Call once per frame.
    pub fn poll(&mut self) -> TrayCommand {
        #[cfg(windows)]
        unsafe {
            // Process any pending messages for our hidden window.
            use windows::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, PeekMessageW, TranslateMessage, PM_REMOVE,
            };
            let mut msg = std::mem::zeroed();
            while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);

                // Check for WM_COMMAND from our menu
                if msg.message == WM_COMMAND {
                    let id = (msg.wParam.0 & 0xFFFF) as u16;
                    self.pending_command = match id {
                        ID_QUIT => TrayCommand::Quit,
                        ID_MODE_WORK => TrayCommand::SetModeWork,
                        ID_MODE_PLAY => TrayCommand::SetModePlay,
                        ID_MODE_ZEN => TrayCommand::SetModeZen,
                        ID_MODE_CHAOS => TrayCommand::SetModeChaos,
                        ID_PAUSE => TrayCommand::TogglePause,
                        ID_DEBUG => TrayCommand::ToggleDebug,
                        _ => TrayCommand::None,
                    };
                }
            }
        }

        let cmd = self.pending_command;
        self.pending_command = TrayCommand::None;
        cmd
    }

    /// Remove the tray icon (called on shutdown).
    pub fn remove(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[cfg(windows)]
impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.remove();
    }
}

/// Window procedure for the hidden tray message window.
#[cfg(windows)]
unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TRAYICON {
        let event = (lparam.0 & 0xFFFF) as u32;
        // WM_RBUTTONUP = 0x0205
        if event == 0x0205 {
            show_context_menu(hwnd);
            return LRESULT(0);
        }
    }
    if msg == WM_COMMAND {
        // Post back to self so poll() picks it up via PeekMessage
        let _ = PostMessageW(hwnd, WM_COMMAND, wparam, LPARAM(0));
        return LRESULT(0);
    }
    if msg == WM_DESTROY {
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Show the right-click context menu at the cursor position.
#[cfg(windows)]
unsafe fn show_context_menu(hwnd: HWND) {
    let hmenu = CreatePopupMenu().expect("failed to create popup menu");

    let items: &[(u16, &str)] = &[
        (ID_MODE_WORK, "Mode: Work"),
        (ID_MODE_PLAY, "Mode: Play"),
        (ID_MODE_ZEN, "Mode: Zen"),
        (ID_MODE_CHAOS, "Mode: Chaos"),
    ];

    for &(id, label) in items {
        let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            id as usize,
            windows::core::PCWSTR(wide.as_ptr()),
        );
    }

    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());

    let pause_label: Vec<u16> = "Pause\0".encode_utf16().collect();
    let _ = AppendMenuW(
        hmenu,
        MF_STRING,
        ID_PAUSE as usize,
        windows::core::PCWSTR(pause_label.as_ptr()),
    );

    let debug_label: Vec<u16> = "Debug Overlay (F12)\0".encode_utf16().collect();
    let _ = AppendMenuW(
        hmenu,
        MF_STRING,
        ID_DEBUG as usize,
        windows::core::PCWSTR(debug_label.as_ptr()),
    );

    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());

    let quit_label: Vec<u16> = "Quit\0".encode_utf16().collect();
    let _ = AppendMenuW(
        hmenu,
        MF_STRING,
        ID_QUIT as usize,
        windows::core::PCWSTR(quit_label.as_ptr()),
    );

    let mut pt = windows::Win32::Foundation::POINT::default();
    let _ = GetCursorPos(&mut pt);

    // Required so menu closes when clicking outside
    let _ = SetForegroundWindow(hwnd);

    let _ = TrackPopupMenu(
        hmenu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );

    let _ = DestroyMenu(hmenu);
}

// Non-windows stub
#[cfg(not(windows))]
impl TrayIcon {
    pub fn new() -> Self {
        Self {
            pending_command: TrayCommand::None,
        }
    }
    pub fn poll(&mut self) -> TrayCommand {
        TrayCommand::None
    }
    pub fn remove(&mut self) {}
}
