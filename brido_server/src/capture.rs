use anyhow::Result;
use scrap::{Capturer, Display};
use std::io::ErrorKind;
use std::time::Duration;

use crate::capture_gdi::GdiCapture;

/// How the screen capture backend is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMethod {
    /// Use GDI first (captures true visible framebuffer). Falls back to DXGI
    /// only if GDI initialisation fails. This is the default because DXGI
    /// Desktop Duplication captures the compositor layer, not the real screen,
    /// which means kiosk apps (SEB, secure browsers) show the desktop
    /// wallpaper behind them instead of their actual content.
    Auto,
    /// Force DXGI Desktop Duplication.
    Dxgi,
    /// Force GDI BitBlt.
    Gdi,
}

impl CaptureMethod {
    pub fn from_env() -> Self {
        match std::env::var("CAPTURE_METHOD")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "dxgi" => CaptureMethod::Dxgi,
            "gdi" => CaptureMethod::Gdi,
            _ => CaptureMethod::Auto,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal backend enum
// ---------------------------------------------------------------------------

enum Backend {
    Dxgi { capturer: Capturer },
    Gdi { capture: GdiCapture },
}

// ---------------------------------------------------------------------------
// Unified ScreenCapture
// ---------------------------------------------------------------------------

pub struct ScreenCapture {
    backend: Backend,
    width: usize,
    height: usize,
}

impl ScreenCapture {
    /// Create a new screen capture with the requested method.
    ///
    /// - `Auto` — tries GDI first (real framebuffer), falls back to DXGI.
    /// - `Gdi`  — force GDI only.
    /// - `Dxgi` — force DXGI only.
    pub fn new(method: CaptureMethod) -> Result<Self> {
        match method {
            CaptureMethod::Dxgi => Self::init_dxgi(),
            CaptureMethod::Gdi => Self::init_gdi(),
            CaptureMethod::Auto => {
                // Prefer GDI — it reads the actual visible screen, so it works
                // even when kiosk/secure-browser apps are in the foreground.
                match Self::init_gdi() {
                    Ok(cap) => {
                        tracing::info!("Auto mode: using GDI (primary)");
                        Ok(cap)
                    }
                    Err(gdi_err) => {
                        tracing::warn!(
                            "Auto mode: GDI init failed ({}), falling back to DXGI",
                            gdi_err
                        );
                        Self::init_dxgi()
                    }
                }
            }
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the active backend label for logging / status display.
    pub fn backend_label(&self) -> &'static str {
        match &self.backend {
            Backend::Dxgi { .. } => "DXGI",
            Backend::Gdi { .. } => "GDI",
        }
    }

    /// Captures a single frame and returns RGB pixel data.
    pub fn capture_frame(&mut self) -> Result<Vec<u8>> {
        match &mut self.backend {
            Backend::Dxgi { capturer } => {
                Self::frame_from_dxgi(capturer, self.width, self.height)
            }
            Backend::Gdi { capture } => capture.capture_frame(),
        }
    }

    // ------------------------------------------------------------------
    // Initialisation helpers
    // ------------------------------------------------------------------

    fn init_dxgi() -> Result<Self> {
        let display = Display::primary()?;
        let width = display.width();
        let height = display.height();
        let capturer = Capturer::new(display)?;
        tracing::info!("DXGI capture backend initialised: {}×{}", width, height);
        Ok(Self {
            backend: Backend::Dxgi { capturer },
            width,
            height,
        })
    }

    fn init_gdi() -> Result<Self> {
        let gdi = GdiCapture::new()?;
        let w = gdi.width();
        let h = gdi.height();
        Ok(Self {
            backend: Backend::Gdi { capture: gdi },
            width: w,
            height: h,
        })
    }

    // ------------------------------------------------------------------
    // DXGI frame grab
    // ------------------------------------------------------------------

    fn frame_from_dxgi(
        capturer: &mut Capturer,
        width: usize,
        height: usize,
    ) -> Result<Vec<u8>> {
        loop {
            match capturer.frame() {
                Ok(frame) => {
                    // scrap returns BGRA with possible row stride padding
                    let stride = frame.len() / height;
                    let mut rgb = Vec::with_capacity(width * height * 3);
                    for y in 0..height {
                        for x in 0..width {
                            let i = y * stride + x * 4;
                            rgb.push(frame[i + 2]); // R
                            rgb.push(frame[i + 1]); // G
                            rgb.push(frame[i]);     // B
                        }
                    }
                    return Ok(rgb);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}
