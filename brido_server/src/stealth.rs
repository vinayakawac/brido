//! Stealth: hide the overlay window from screen capture, OBS, screen share,
//! and — critically — from browser tab-switching detection.
//!
//! Uses:
//! - `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` — invisible to capture
//! - `WS_EX_TOOLWINDOW` — hidden from Alt-Tab
//! - `WS_EX_NOACTIVATE` — clicking the overlay does NOT steal focus from the
//!   browser, so `visibilitychange` / `blur` events never fire
//! - `WM_MOUSEACTIVATE → MA_NOACTIVATE` subclass — belt-and-suspenders defense

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY,
    GetWindowLongW, SetWindowLongW, GWL_EXSTYLE,
    WS_EX_TOOLWINDOW, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    GetForegroundWindow, SetForegroundWindow,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};

/// `WDA_EXCLUDEFROMCAPTURE` (0x00000011) — the window will not be captured by
/// PrintScreen, BitBlt, DWM thumbnail APIs, or any screen-sharing software.
const WDA_EXCLUDEFROMCAPTURE: WINDOW_DISPLAY_AFFINITY = WINDOW_DISPLAY_AFFINITY(0x00000011);

/// `WM_MOUSEACTIVATE` — sent when the user clicks in an inactive window.
const WM_MOUSEACTIVATE: u32 = 0x0021;

/// `MA_NOACTIVATE` — return this from WM_MOUSEACTIVATE to prevent activation.
const MA_NOACTIVATE: isize = 3;

/// Subclass ID for our WM_MOUSEACTIVATE hook.
const SUBCLASS_ID: usize = 0xB01D0;

/// Apply stealth to a window so it is:
/// 1. Excluded from all screen capture APIs
/// 2. Hidden from the Alt-Tab menu
/// 3. Non-activating (clicking it does NOT steal focus from the browser)
///
/// # Safety
/// Requires a valid HWND.  The calls are safe Win32 property-sets.
pub fn apply_stealth(hwnd: isize) {
    unsafe {
        let handle = HWND(hwnd as *mut _);
        
        // ── 1. Hide from screen capture ─────────────────────────────────
        let result = SetWindowDisplayAffinity(handle, WDA_EXCLUDEFROMCAPTURE);
        if let Err(e) = result {
            tracing::warn!("SetWindowDisplayAffinity failed: {e}");
        } else {
            tracing::info!("Stealth: window excluded from screen capture");
        }

        // ── 2. Window style flags ───────────────────────────────────────
        // WS_EX_TOOLWINDOW   — hides from Alt-Tab
        // WS_EX_NOACTIVATE   — clicking the overlay does NOT move focus
        //                      away from the browser, preventing
        //                      visibilitychange / blur detection
        // Remove WS_EX_APPWINDOW just in case.
        let ex_style = GetWindowLongW(handle, GWL_EXSTYLE);
        if ex_style != 0 {
            let mut new_style = ex_style as u32;
            new_style |= WS_EX_TOOLWINDOW.0;
            new_style |= WS_EX_NOACTIVATE.0;
            new_style &= !WS_EX_APPWINDOW.0;
            
            let _ = SetWindowLongW(handle, GWL_EXSTYLE, new_style as i32);
            tracing::info!(
                "Stealth: WS_EX_TOOLWINDOW + WS_EX_NOACTIVATE applied \
                 (window won't steal focus from browser)"
            );
        }

        // ── 3. Subclass to intercept WM_MOUSEACTIVATE ──────────────────
        // Even with WS_EX_NOACTIVATE, some edge-cases can slip through.
        // Returning MA_NOACTIVATE from WM_MOUSEACTIVATE is the definitive
        // way to prevent a window from becoming the foreground window.
        let ok = SetWindowSubclass(handle, Some(noactivate_subclass_proc), SUBCLASS_ID, 0);
        if ok.as_bool() {
            tracing::info!("Stealth: WM_MOUSEACTIVATE subclass installed");
        } else {
            tracing::warn!("Stealth: SetWindowSubclass failed — focus-steal protection may be weaker");
        }
    }
}

/// Save the currently focused window's HWND so we can restore focus later.
/// Call this *before* any operation that might temporarily steal focus.
pub fn get_foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

/// Restore focus to a previously saved foreground window.
/// This is used after text input submission to give focus back to the browser.
pub fn restore_focus(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let handle = HWND(hwnd as *mut _);
        let _ = SetForegroundWindow(handle);
    }
}

/// Subclass procedure that intercepts `WM_MOUSEACTIVATE` and returns
/// `MA_NOACTIVATE`, preventing the window from becoming active on click.
unsafe extern "system" fn noactivate_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid_subclass: usize,
    _dw_ref_data: usize,
) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
