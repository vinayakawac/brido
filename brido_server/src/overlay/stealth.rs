//! Stealth: hide the overlay window from screen capture, OBS, and screen share.
//!
//! Uses the Win32 `SetWindowDisplayAffinity` API with
//! `WDA_EXCLUDEFROMCAPTURE` — the same mechanism ShadowPrep uses.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY,
};

/// `WDA_EXCLUDEFROMCAPTURE` (0x00000011) — the window will not be captured by
/// PrintScreen, BitBlt, DWM thumbnail APIs, or any screen-sharing software.
const WDA_EXCLUDEFROMCAPTURE: WINDOW_DISPLAY_AFFINITY = WINDOW_DISPLAY_AFFINITY(0x00000011);

/// Apply stealth to a window so it is excluded from all screen capture APIs.
///
/// # Safety
/// Requires a valid HWND.  The call itself is safe — it is a single Win32
/// property-set that cannot corrupt memory.
pub fn apply_stealth(hwnd: isize) {
    unsafe {
        let handle = HWND(hwnd as *mut _);
        let result = SetWindowDisplayAffinity(handle, WDA_EXCLUDEFROMCAPTURE);
        if let Err(e) = result {
            tracing::warn!("SetWindowDisplayAffinity failed: {e}");
        } else {
            tracing::info!("Stealth: window excluded from screen capture");
        }
    }
}

