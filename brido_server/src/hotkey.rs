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
    OpenSettings,
}

const HOTKEY_CAPTURE: i32 = 1;
const HOTKEY_TOGGLE: i32 = 2;
const HOTKEY_SETTINGS: i32 = 3;

/// Parses a hotkey string (e.g. "Ctrl+Shift+Space", "Ctrl+`") into modifiers and a Virtual Key Code.
pub fn parse_hotkey(s: &str) -> (HOT_KEY_MODIFIERS, u32) {
    let mut mods = MOD_NOREPEAT;
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    
    // If no modifiers specified, default to Ctrl+Shift for backwards compatibility
    if parts.len() == 1 {
        mods |= MOD_CONTROL | MOD_SHIFT;
    } else {
        for &part in &parts[..parts.len() - 1] {
            let p = part.to_uppercase();
            if p == "CTRL" || p == "CONTROL" {
                mods |= MOD_CONTROL;
            } else if p == "SHIFT" {
                mods |= MOD_SHIFT;
            } else if p == "ALT" {
                mods |= windows::Win32::UI::Input::KeyboardAndMouse::MOD_ALT;
            } else if p == "WIN" || p == "WINDOWS" {
                mods |= windows::Win32::UI::Input::KeyboardAndMouse::MOD_WIN;
            }
        }
    }

    let k = parts.last().unwrap_or(&"SPACE").to_uppercase();
    let vk = match k.as_str() {
        "SPACE" => 0x20,
        "`" | "~" | "BACKTICK" => 0xC0, // VK_OEM_3
        "[" => 0xDB, // VK_OEM_4
        "]" => 0xDD, // VK_OEM_6
        "," => 0xBC, // VK_OEM_COMMA
        "." => 0xBE, // VK_OEM_PERIOD
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
            if k.len() == 1 {
                let c = k.chars().next().unwrap();
                if c >= 'A' && c <= 'Z' {
                    return (mods, c as u32);
                }
            }
            0x20 // default to space if invalid
        }
    };
    (mods, vk)
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
    vk_settings_str: &str,
) -> (std::thread::JoinHandle<()>, HotkeyHandle) {
    let (mod_capture, vk_capture) = parse_hotkey(vk_capture_str);
    let (mod_toggle, vk_toggle) = parse_hotkey(vk_toggle_str);
    let (mod_settings, vk_settings) = parse_hotkey(vk_settings_str);
    
    // We clone for logging inside the thread
    let cap_str = vk_capture_str.to_string();
    let tog_str = vk_toggle_str.to_string();
    let set_str = vk_settings_str.to_string();
    
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

            if let Err(e) = RegisterHotKey(None, HOTKEY_CAPTURE, mod_capture, vk_capture) {
                tracing::error!("Failed to register capture hotkey (VK {}): {e}", vk_capture);
            } else {
                tracing::info!("Registered hotkey: {} → capture & analyse", cap_str);
            }

            if let Err(e) = RegisterHotKey(None, HOTKEY_TOGGLE, mod_toggle, vk_toggle) {
                tracing::error!("Failed to register toggle hotkey (VK {}): {e}", vk_toggle);
            } else {
                tracing::info!("Registered hotkey: {} → toggle visibility", tog_str);
            }

            if let Err(e) = RegisterHotKey(None, HOTKEY_SETTINGS, mod_settings, vk_settings) {
                tracing::error!("Failed to register settings hotkey (VK {}): {e}", vk_settings);
            } else {
                tracing::info!("Registered hotkey: {} → open settings", set_str);
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
                        HOTKEY_SETTINGS => Some(OverlayEvent::OpenSettings),
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
            let _ = UnregisterHotKey(None, HOTKEY_SETTINGS);
            tracing::info!("Hotkey listener stopped");
        }
    });

    let thread_id = tid_rx.recv().unwrap_or(0);
    (join_handle, HotkeyHandle { thread_id })
}
