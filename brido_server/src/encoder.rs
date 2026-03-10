use anyhow::Result;
use image::{codecs::jpeg::JpegEncoder, DynamicImage, RgbImage};
use std::io::Cursor;

pub struct FrameEncoder {
    target_width: u32,
    target_height: u32,
    quality: u8,
}

impl FrameEncoder {
    pub fn new(target_width: u32, target_height: u32, quality: u8) -> Self {
        Self {
            target_width,
            target_height,
            quality,
        }
    }

    /// Encodes raw RGB pixel data into a JPEG buffer.
    /// Resizes to the target resolution while maintaining aspect ratio.
    pub fn encode(&self, rgb_data: &[u8], src_width: u32, src_height: u32) -> Result<Vec<u8>> {
        let img = RgbImage::from_raw(src_width, src_height, rgb_data.to_vec())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from raw RGB data"))?;

        let dynamic = DynamicImage::ImageRgb8(img);

        let resized = if src_width != self.target_width || src_height != self.target_height {
            dynamic.resize(
                self.target_width,
                self.target_height,
                image::imageops::FilterType::Triangle,
            )
        } else {
            dynamic
        };

        let mut buf = Cursor::new(Vec::new());
        let encoder = JpegEncoder::new_with_quality(&mut buf, self.quality);
        resized.write_with_encoder(encoder)?;

        Ok(buf.into_inner())
    }
}
