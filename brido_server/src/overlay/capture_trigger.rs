//! On-demand screen capture for the overlay.
//!
//! Captures a single frame, encodes it as JPEG, and returns a base64 string
//! ready for AI provider consumption.

use anyhow::Result;
use base64::Engine as _;
use brido_server::capture::{CaptureMethod, ScreenCapture};
use brido_server::encoder::FrameEncoder;

/// Capture the current screen and return it as a base64-encoded JPEG string.
///
/// Uses higher resolution and quality than the streaming path since this is
/// a single-shot analysis capture, not a continuous stream.
pub fn capture_screen_base64() -> Result<String> {
    let capture_method = CaptureMethod::from_env();
    let mut cap = ScreenCapture::new(capture_method)?;

    let rgb = cap.capture_frame()?;
    let src_w = cap.width() as u32;
    let src_h = cap.height() as u32;

    // Higher quality single-shot capture (server streams at 1280×720 q65)
    let encoder = FrameEncoder::new(1440, 900, 88);
    let jpeg = encoder.encode(&rgb, src_w, src_h)?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg);
    Ok(b64)
}
