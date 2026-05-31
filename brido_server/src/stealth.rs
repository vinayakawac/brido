//! Stealth: hide the overlay window from screen capture, OBS, and screen share.
//!
//! Uses the Win32 `SetWindowDisplayAffinity` API with
//! `WDA_EXCLUDEFROMCAPTURE` — the same mechanism ShadowPrep uses.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY,
    GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_TOOLWINDOW, WS_EX_APPWINDOW
};

/// `WDA_EXCLUDEFROMCAPTURE` (0x00000011) — the window will not be captured by
/// PrintScreen, BitBlt, DWM thumbnail APIs, or any screen-sharing software.
const WDA_EXCLUDEFROMCAPTURE: WINDOW_DISPLAY_AFFINITY = WINDOW_DISPLAY_AFFINITY(0x00000011);

/// Apply stealth to a window so it is excluded from all screen capture APIs
/// and hidden from the Alt-Tab menu.
///
/// # Safety
/// Requires a valid HWND.  The call itself is safe — it is a single Win32
/// property-set that cannot corrupt memory.
pub fn apply_stealth(hwnd: isize) {
    unsafe {
        let handle = HWND(hwnd as *mut _);
        
        // Hide from screen capture
        let result = SetWindowDisplayAffinity(handle, WDA_EXCLUDEFROMCAPTURE);
        if let Err(e) = result {
            tracing::warn!("SetWindowDisplayAffinity failed: {e}");
        } else {
            tracing::info!("Stealth: window excluded from screen capture");
        }

        // Hide from Alt-Tab by applying WS_EX_TOOLWINDOW
        let ex_style = GetWindowLongW(handle, GWL_EXSTYLE);
        if ex_style != 0 {
            // WS_EX_TOOLWINDOW hides from Alt-Tab. We also remove WS_EX_APPWINDOW just in case.
            let mut new_style = ex_style as u32;
            new_style |= WS_EX_TOOLWINDOW.0;
            new_style &= !WS_EX_APPWINDOW.0;
            
            let _ = SetWindowLongW(handle, GWL_EXSTYLE, new_style as i32);
            tracing::info!("Stealth: window hidden from Alt-Tab (WS_EX_TOOLWINDOW applied)");
        }
    }
}

