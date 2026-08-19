//! Remote keyboard: type text from the phone into whatever window currently
//! has focus on this PC.
//!
//! This is a plain input-injection helper — the same thing a Bluetooth keyboard
//! or an on-screen keyboard does. It makes no attempt to hide itself, evade
//! monitoring software, or disguise where the input came from; it simply
//! synthesises Unicode key events for the focused application.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT,
    VK_TAB, VK_UP,
};

/// Named editing keys the phone can send, so the remote keyboard can move the
/// caret and delete rather than only appending text.
pub fn press_named_key(name: &str) -> bool {
    let vk = match name.to_ascii_lowercase().as_str() {
        "backspace" => VK_BACK.0,
        "delete" => VK_DELETE.0,
        "left" => VK_LEFT.0,
        "right" => VK_RIGHT.0,
        "up" => VK_UP.0,
        "down" => VK_DOWN.0,
        "home" => VK_HOME.0,
        "end" => VK_END.0,
        "enter" => VK_RETURN.0,
        "tab" => VK_TAB.0,
        _ => return false,
    };
    send_pair(
        make_input_vk(vk, KEYBD_EVENT_FLAGS(0)),
        make_input_vk(vk, KEYEVENTF_KEYUP),
    );
    true
}

/// Types `text` into the focused window as a sequence of Unicode key events.
///
/// Newlines become Enter, tabs become Tab, and everything else is sent as a
/// literal Unicode character (so emoji and non-Latin scripts work). Returns the
/// number of characters that were dispatched.
///
/// Each character is sent as its own down+up `SendInput` call rather than one
/// big batch: batching many events into a single call makes some target apps
/// auto-repeat characters (typing "from" came out as "oooo"). One call per
/// character is a touch slower but types exactly what was sent.
pub fn type_text(text: &str) -> usize {
    let mut count = 0usize;
    for ch in text.chars() {
        match ch {
            '\n' | '\r' => send_pair(make_input_vk(VK_RETURN.0, KEYBD_EVENT_FLAGS(0)),
                                     make_input_vk(VK_RETURN.0, KEYEVENTF_KEYUP)),
            '\t' => send_pair(make_input_vk(VK_TAB.0, KEYBD_EVENT_FLAGS(0)),
                              make_input_vk(VK_TAB.0, KEYEVENTF_KEYUP)),
            _ => {
                // A char may be outside the BMP (e.g. emoji); each UTF-16 code
                // unit is sent as its own Unicode event.
                let mut buf = [0u16; 2];
                for unit in ch.encode_utf16(&mut buf) {
                    send_pair(
                        make_input(*unit, KEYEVENTF_UNICODE),
                        make_input(*unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                    );
                }
            }
        }
        count += 1;
    }
    count
}

/// Sends a single Backspace, so the phone's delete key can edit at the cursor.
pub fn backspace(times: usize) {
    for _ in 0..times {
        send_pair(
            make_input_vk(VK_BACK.0, KEYBD_EVENT_FLAGS(0)),
            make_input_vk(VK_BACK.0, KEYEVENTF_KEYUP),
        );
    }
}

/// Dispatches one key-down followed by its key-up.
fn send_pair(down: INPUT, up: INPUT) {
    let events = [down, up];
    // SAFETY: `events` is a valid slice of INPUT alive for the call.
    unsafe {
        SendInput(&events, std::mem::size_of::<INPUT>() as i32);
    }
}

fn make_input(scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn make_input_vk(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
