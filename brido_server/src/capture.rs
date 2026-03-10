use anyhow::Result;
use scrap::{Capturer, Display};
use std::io::ErrorKind;
use std::time::Duration;

pub struct ScreenCapture {
    capturer: Capturer,
    width: usize,
    height: usize,
}

impl ScreenCapture {
    pub fn new() -> Result<Self> {
        let display = Display::primary()?;
        let width = display.width();
        let height = display.height();
        let capturer = Capturer::new(display)?;
        Ok(Self {
            capturer,
            width,
            height,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Captures a single frame and returns RGB pixel data.
    /// Blocks briefly if no new frame is available (DXGI WouldBlock).
    pub fn capture_frame(&mut self) -> Result<Vec<u8>> {
        loop {
            match self.capturer.frame() {
                Ok(frame) => {
                    // scrap returns BGRA with possible row stride padding
                    let stride = frame.len() / self.height;
                    let mut rgb = Vec::with_capacity(self.width * self.height * 3);
                    for y in 0..self.height {
                        for x in 0..self.width {
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
