use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use qrcode::{QrCode, EcLevel};

/// Generates a QR code texture from a `brido://` payload string.
pub fn generate_qr_texture(
    ctx: &egui::Context,
    payload: &str,
    prev: Option<TextureHandle>,
) -> TextureHandle {
    // Drop old texture
    drop(prev);

    let code = QrCode::with_error_correction_level(payload, EcLevel::M)
        .expect("QR code generation failed");

    let modules = code.to_colors();
    let width = code.width();
    let scale = 6; // pixels per module
    let margin = 2; // module-width quiet zone
    let img_size = (width + margin * 2) * scale;

    let mut pixels = vec![Color32::WHITE; img_size * img_size];

    for y in 0..width {
        for x in 0..width {
            let dark = modules[y * width + x] == qrcode::Color::Dark;
            if dark {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = (x + margin) * scale + dx;
                        let py = (y + margin) * scale + dy;
                        pixels[py * img_size + px] = Color32::BLACK;
                    }
                }
            }
        }
    }

    let image = ColorImage {
        size: [img_size, img_size],
        pixels,
    };

    ctx.load_texture("qr_code", image, TextureOptions::NEAREST)
}
