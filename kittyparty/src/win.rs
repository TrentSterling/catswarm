//! Win32 bits: the click-through overlay, the tray icon and its menu,
//! start-with-Windows, global input polling and a single-instance lock.
//! Overlay styling mirrors catswarm's `platform/win32.rs`.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::atomic::{AtomicU32, Ordering};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn hwnd_of(window: &winit::window::Window) -> HWND {
    match window.window_handle().expect("window handle").as_raw() {
        RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut core::ffi::c_void),
        _ => panic!("expected a Win32 window"),
    }
}

/// Turn the winit window into a transparent, click-through, always-on-top
/// overlay that never takes focus or shows up in Alt+Tab.
pub fn make_overlay(window: &winit::window::Window) {
    let hwnd = hwnd_of(window);
    unsafe {
        apply_styles(hwnd);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        // No DWM border, rounded corners or Mica behind the overlay.
        for (attr, value) in [(2u32, 2u32), (33, 1), (34, 0xFFFF_FFFE), (38, 1)] {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(attr as i32),
                &value as *const u32 as *const core::ffi::c_void,
                4,
            );
        }
    }
}

/// How the overlay lets clicks through. Plain WS_EX_TRANSPARENT (catswarm's
/// and DuckToy's style) still takes hits from other processes, so the default
/// is the classic layered + transparent pair with a fully opaque layer alpha,
/// which keeps the DirectComposition visual on screen.
/// KITTYPARTY_CLICK_MODE=plain switches back for testing.
fn layered_mode() -> bool {
    static MODE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| std::env::var("KITTYPARTY_CLICK_MODE").map(|v| v != "plain").unwrap_or(true))
}

/// No GDI redirection surface: all pixels come from the DirectComposition
/// visual wgpu creates (DxgiFromVisual). Cheap when nothing changed, so it
/// runs every frame: winit rewrites the extended style whenever it applies
/// its own cached window flags (on show, focus changes and so on).
pub fn apply_styles(hwnd: HWND) {
    const WS_EX_NOREDIRECTIONBITMAP: isize = 0x0020_0000;
    unsafe {
        let old = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let mut new = (old & !(WS_EX_APPWINDOW.0 as isize | WS_EX_LAYERED.0 as isize))
            | WS_EX_NOACTIVATE.0 as isize
            | WS_EX_TOOLWINDOW.0 as isize
            | WS_EX_TRANSPARENT.0 as isize
            | WS_EX_NOREDIRECTIONBITMAP;
        if layered_mode() {
            new |= WS_EX_LAYERED.0 as isize;
        }
        if old != new {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
            if layered_mode() {
                let _ = SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 255, LWA_ALPHA);
            }
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

/// Other topmost apps can push us under them; re-assert now and then.
pub fn keep_on_top(window: &winit::window::Window) {
    unsafe {
        let _ = SetWindowPos(
            hwnd_of(window),
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

pub fn cursor_pos() -> (f32, f32) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x as f32, p.y as f32)
}

fn key_down(vk: i32) -> bool {
    unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
}

pub fn left_button_down() -> bool {
    key_down(0x01)
}

/// Ctrl+Alt+K toggles the party on and off.
pub fn hotkey_down() -> bool {
    key_down(0x11) && key_down(0x12) && key_down(0x4B)
}

/// Only one KittyParty at a time; a second launch just exits.
pub fn already_running() -> bool {
    unsafe {
        let handle = CreateMutexW(None, true, w!("Local\\KittyPartySingleInstance"));
        // Leak the handle on purpose: the mutex must live as long as the process.
        std::mem::forget(handle);
        GetLastError() == ERROR_ALREADY_EXISTS
    }
}

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("KittyParty");

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn autostart_enabled() -> bool {
    let mut size = 0u32;
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE, RRF_RT_REG_SZ, None, None, Some(&mut size)).is_ok()
    }
}

pub fn set_autostart(on: bool) {
    unsafe {
        if on {
            let Ok(exe) = std::env::current_exe() else { return };
            let cmd = wide(&format!("\"{}\"", exe.display()));
            let _ = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                RUN_VALUE,
                REG_SZ.0,
                Some(cmd.as_ptr() as *const core::ffi::c_void),
                (cmd.len() * 2) as u32,
            );
        } else {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE);
        }
    }
}

pub fn open_folder(path: &std::path::Path) {
    let p = wide(&path.display().to_string());
    unsafe {
        ShellExecuteW(HWND::default(), w!("open"), PCWSTR(p.as_ptr()), None, None, SW_SHOWNORMAL);
    }
}

// ---------------------------------------------------------------------------
// Tray icon
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    TogglePause,
    SetRain(u32),
    ToggleClickBursts,
    BurstNow,
    ToggleAutostart,
    OpenFolder,
    Quit,
}

const WM_TRAY: u32 = WM_USER + 1;
const ID_PAUSE: u32 = 1;
const ID_RAIN_BASE: u32 = 10; // 10..=12
const ID_CLICK: u32 = 20;
const ID_BURST: u32 = 21;
const ID_AUTOSTART: u32 = 22;
const ID_FOLDER: u32 = 23;
const ID_QUIT: u32 = 99;

/// Menu choice waiting to be picked up by the app (0 = none).
static PENDING: AtomicU32 = AtomicU32::new(0);
/// Menu check marks, mirrored from the app each frame.
pub static MENU_PAUSED: AtomicU32 = AtomicU32::new(0);
pub static MENU_RAIN: AtomicU32 = AtomicU32::new(1);
pub static MENU_CLICK: AtomicU32 = AtomicU32::new(1);

pub struct Tray {
    nid: NOTIFYICONDATAW,
}

impl Tray {
    /// `icon_rgba` is a 32x32 straight-alpha RGBA image.
    pub fn new(icon_rgba: &[u8]) -> Option<Self> {
        unsafe {
            let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).ok()?;
            let class = w!("KittyPartyTray");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(tray_proc),
                hInstance: hinstance.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            // A hidden top-level window (not HWND_MESSAGE) so it also receives
            // the TaskbarCreated broadcast if Explorer restarts.
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!("KittyParty"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                None,
                None,
                hinstance,
                None,
            )
            .ok()?;

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                ..Default::default()
            };
            nid.hIcon = make_icon(icon_rgba).unwrap_or_else(|| LoadIconW(None, IDI_APPLICATION).unwrap_or_default());
            for (i, ch) in "KittyParty - right-click for options".encode_utf16().enumerate().take(nid.szTip.len() - 1) {
                nid.szTip[i] = ch;
            }
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
            TRAY_NID.with(|t| *t.borrow_mut() = Some(nid));
            Some(Self { nid })
        }
    }

    pub fn poll(&self) -> Option<TrayCommand> {
        Some(match PENDING.swap(0, Ordering::AcqRel) {
            0 => return None,
            ID_PAUSE => TrayCommand::TogglePause,
            id @ ID_RAIN_BASE..=12 => TrayCommand::SetRain(id - ID_RAIN_BASE),
            ID_CLICK => TrayCommand::ToggleClickBursts,
            ID_BURST => TrayCommand::BurstNow,
            ID_AUTOSTART => TrayCommand::ToggleAutostart,
            ID_FOLDER => TrayCommand::OpenFolder,
            ID_QUIT => TrayCommand::Quit,
            _ => return None,
        })
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
        }
    }
}

thread_local! {
    static TRAY_NID: std::cell::RefCell<Option<NOTIFYICONDATAW>> = const { std::cell::RefCell::new(None) };
}

/// Build an HICON from RGBA pixels (alpha is honoured on Vista+).
fn make_icon(rgba: &[u8]) -> Option<HICON> {
    let side = 32usize;
    if rgba.len() != side * side * 4 {
        return None;
    }
    let mut bgra = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        bgra.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    let and_mask = vec![0u8; side * side / 8];
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).ok()?;
        CreateIcon(hinstance, side as i32, side as i32, 1, 32, and_mask.as_ptr(), bgra.as_ptr()).ok()
    }
}

unsafe extern "system" fn tray_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
    if TASKBAR_CREATED.load(Ordering::Relaxed) == 0 {
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);
    }
    match msg {
        WM_TRAY => {
            let event = (lparam.0 & 0xFFFF) as u32;
            // Left or right click both open the menu; easier to discover.
            if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                show_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            PENDING.store((wparam.0 & 0xFFFF) as u32, Ordering::Release);
            LRESULT(0)
        }
        m if m == TASKBAR_CREATED.load(Ordering::Relaxed) && m != 0 => {
            TRAY_NID.with(|t| {
                if let Some(nid) = t.borrow().as_ref() {
                    let _ = Shell_NotifyIconW(NIM_ADD, nid);
                }
            });
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else { return };
    let add = |id: u32, label: &str, checked: bool| {
        let text = wide(label);
        let flags = if checked { MF_STRING | MF_CHECKED } else { MF_STRING };
        let _ = AppendMenuW(menu, flags, id as usize, PCWSTR(text.as_ptr()));
    };
    let sep = || {
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    };

    add(ID_BURST, "Cat burst now!", false);
    sep();
    let rain = MENU_RAIN.load(Ordering::Relaxed);
    add(ID_RAIN_BASE, "Cat rain: gentle", rain == 0);
    add(ID_RAIN_BASE + 1, "Cat rain: cozy", rain == 1);
    add(ID_RAIN_BASE + 2, "Cat rain: party!", rain == 2);
    sep();
    add(ID_CLICK, "Clicking makes cats pop", MENU_CLICK.load(Ordering::Relaxed) != 0);
    add(ID_PAUSE, "Hide kitty (Ctrl+Alt+K)", MENU_PAUSED.load(Ordering::Relaxed) != 0);
    add(ID_AUTOSTART, "Start with Windows", autostart_enabled());
    add(ID_FOLDER, "Add my own pictures...", false);
    sep();
    add(ID_QUIT, "Quit", false);

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    // Needed so the menu closes when she clicks elsewhere.
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, None);
    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    let _ = DestroyMenu(menu);
}
