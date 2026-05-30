//! Global hotkey registration using Win32 `RegisterHotKey`.
//!
//! Runs a Windows message loop in a background thread and posts events
//! to the main overlay thread via an mpsc channel.

use std::sync::mpsc;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_QUIT};

/// Events posted from the hotkey listener to the overlay main thread.
#[derive(Debug, Clone, Copy)]
pub enum OverlayEvent {
    CaptureAndAnalyse,
    ToggleVisibility,
}

const HOTKEY_CAPTURE: i32 = 1;
const HOTKEY_TOGGLE: i32 = 2;

/// Convert a simple string (e.g. "Space", "H", "A") to a Virtual Key Code.
pub fn vk_from_string(s: &str) -> u32 {
    let s = s.trim().to_uppercase();
    match s.as_str() {
        "SPACE" => 0x20,
        "0" => 0x30,
        "1" => 0x31,
        "2" => 0x32,
        "3" => 0x33,
        "4" => 0x34,
        "5" => 0x35,
        "6" => 0x36,
        "7" => 0x37,
        "8" => 0x38,
        "9" => 0x39,
        _ => {
            if s.len() == 1 {
                let c = s.chars().next().unwrap();
                if c >= 'A' && c <= 'Z' {
                    return c as u32;
                }
            }
            0x20 // default to space if invalid
        }
    }
}

pub struct HotkeyHandle {
    thread_id: u32,
}

impl HotkeyHandle {
    /// Sends WM_QUIT to the hotkey listener thread, causing GetMessageW to exit.
    pub fn stop(&self) {
        unsafe {
            let _ = PostThreadMessageW(
                self.thread_id,
                WM_QUIT,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }
    }
}

/// Spawn a background thread that listens for global hotkeys.
///
/// Returns a join handle and a HotkeyHandle used to stop the thread.
pub fn start_hotkey_listener(
    tx: mpsc::Sender<OverlayEvent>,
    vk_capture_str: &str,
    vk_toggle_str: &str,
) -> (std::thread::JoinHandle<()>, HotkeyHandle) {
    let vk_capture = vk_from_string(vk_capture_str);
    let vk_toggle = vk_from_string(vk_toggle_str);
    
    // We clone vk_capture_str and vk_toggle_str for logging inside the thread
    let cap_str = vk_capture_str.to_string();
    let tog_str = vk_toggle_str.to_string();
    
    let (tid_tx, tid_rx) = mpsc::channel();
    
    let join_handle = std::thread::spawn(move || {
        unsafe {
            let thread_id = GetCurrentThreadId();
            if tid_tx.send(thread_id).is_err() {
                return;
            }

            // Force message queue creation for this thread by peeking once
            let mut dummy = MSG::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                &mut dummy, None, 0, 0, windows::Win32::UI::WindowsAndMessaging::PM_NOREMOVE
            );

            let modifiers: HOT_KEY_MODIFIERS = MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT;

            if let Err(e) = RegisterHotKey(None, HOTKEY_CAPTURE, modifiers, vk_capture) {
                tracing::error!("Failed to register capture hotkey (VK {}): {e}", vk_capture);
            } else {
                tracing::info!("Registered hotkey: Ctrl+Shift+{} → capture & analyse", cap_str);
            }

            if let Err(e) = RegisterHotKey(None, HOTKEY_TOGGLE, modifiers, vk_toggle) {
                tracing::error!("Failed to register toggle hotkey (VK {}): {e}", vk_toggle);
            } else {
                tracing::info!("Registered hotkey: Ctrl+Shift+{} → toggle visibility", tog_str);
            }

            let mut msg = MSG::default();
            loop {
                let ret = GetMessageW(&mut msg, None, 0, 0);
                // GetMessageW returns 0 for WM_QUIT, -1 on error
                if ret.0 == 0 || ret.0 == -1 {
                    break;
                }

                if msg.message == WM_HOTKEY {
                    let id = msg.wParam.0 as i32;
                    let event = match id {
                        HOTKEY_CAPTURE => Some(OverlayEvent::CaptureAndAnalyse),
                        HOTKEY_TOGGLE => Some(OverlayEvent::ToggleVisibility),
                        _ => None,
                    };

                    if let Some(evt) = event {
                        if tx.send(evt).is_err() {
                            break;
                        }
                    }
                }
            }

            let _ = UnregisterHotKey(None, HOTKEY_CAPTURE);
            let _ = UnregisterHotKey(None, HOTKEY_TOGGLE);
            tracing::info!("Hotkey listener stopped");
        }
    });

    let thread_id = tid_rx.recv().unwrap_or(0);
    (join_handle, HotkeyHandle { thread_id })
}
