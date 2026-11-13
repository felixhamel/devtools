//! QR Code Reader/Generator.
use eframe::egui;
use super::{Category, Tool};
use crate::ui;

/// Pixels per QR module in the generated image, and the quiet zone width in modules.
const SCALE: usize = 8;
const QUIET: usize = 4;

/// Generates a QR code live from the text, and reads QR codes from a pasted or dropped image.
#[derive(Default)]
pub struct Qr { text: String, rendered: Option<String>, image: Option<Rgba>, texture: Option<egui::TextureHandle>, error: String }

#[derive(Clone)]
struct Rgba { width: usize, height: usize, pixels: Vec<u8> }

impl Qr {
    fn read(&mut self, image: &Rgba) {
        match decode(image) { Ok(text) => { self.text = text; self.error.clear(); } Err(e) => self.error = e }
    }

    fn read_file(&mut self, path: &std::path::Path) {
        match image::open(path) {
            Ok(img) => { let img = img.into_rgba8(); let (w, h) = img.dimensions(); self.read(&Rgba { width: w as usize, height: h as usize, pixels: img.into_raw() }); }
            Err(e) => self.error = format!("Cannot open {}: {e}", path.display()),
        }
    }
}

impl Tool for Qr {
    fn name(&self) -> &'static str { "QR Code Reader/Generator" }
    fn badge(&self) -> &'static str { "QR" }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        for file in ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(path) = &file.path { self.read_file(path); }
        }
        // Regenerate the QR code only when the text changed since the last render.
        if self.rendered.as_deref() != Some(self.text.as_str()) {
            self.rendered = Some(self.text.clone());
            self.texture = None;
            self.image = None;
            if !self.text.is_empty() {
                match encode(&self.text) {
                    Ok(image) => {
                        let color = egui::ColorImage::from_rgba_unmultiplied([image.width, image.height], &image.pixels);
                        self.texture = Some(ctx.load_texture("qr", color, egui::TextureOptions::NEAREST));
                        self.image = Some(image);
                        self.error.clear();
                    }
                    Err(e) => self.error = e,
                }
            }
        }
        ui::description(ui, "Type text to generate a QR code. To read one, paste an image (e.g. a screenshot) or drop an image file on the window.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            ui::toolbar(left, "Content:", |ui| {
                if ui.add(ui::action_button("Clipboard", false)).clicked() {
                    match ui::clipboard_text() { Ok(text) => self.text = text, Err(e) => self.error = e }
                }
                if ui.add(ui::action_button("Read image from clipboard", true)).clicked() {
                    match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
                        Ok(img) => self.read(&Rgba { width: img.width, height: img.height, pixels: img.bytes.into_owned() }),
                        Err(e) => self.error = format!("No image in the clipboard ({e})"),
                    }
                }
                if ui.add(ui::action_button("Clear", false)).clicked() { *self = Self::default(); }
            });
            ui::fill_editor(left, "content", &mut self.text, "https://example.com");
            ui::toolbar(right, "QR code:", |ui| {
                if ui.add_enabled(self.image.is_some(), ui::action_button("Copy image", false)).clicked() {
                    if let Some(img) = &self.image {
                        let data = arboard::ImageData { width: img.width, height: img.height, bytes: std::borrow::Cow::Borrowed(&img.pixels) };
                        if let Err(e) = arboard::Clipboard::new().and_then(|mut c| c.set_image(data)) { self.error = format!("Clipboard: {e}"); }
                    }
                }
            });
            ui::error_label(right, &self.error);
            ui::editor_frame().show(right, |ui| {
                let size = ui.available_size();
                ui.set_min_size(size);
                if let Some(texture) = &self.texture {
                    let side = size.x.min(size.y).min(420.0);
                    ui.vertical_centered(|ui| { ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(side, side))); });
                }
            });
        });
    }
}

/// Renders `text` as a black-on-white QR code with a quiet zone, `SCALE` px per module.
fn encode(text: &str) -> Result<Rgba, String> {
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|e| format!("Cannot encode: {e}"))?;
    let modules = code.width();
    let colors = code.to_colors();
    let side = (modules + 2 * QUIET) * SCALE;
    let mut pixels = vec![255u8; side * side * 4];
    for (i, color) in colors.iter().enumerate() {
        if *color != qrcode::Color::Dark { continue; }
        let (mx, my) = (i % modules + QUIET, i / modules + QUIET);
        for y in my * SCALE..(my + 1) * SCALE {
            for x in mx * SCALE..(mx + 1) * SCALE {
                let p = (y * side + x) * 4;
                pixels[p..p + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
    }
    Ok(Rgba { width: side, height: side, pixels })
}

/// Finds and decodes the first QR code in an RGBA image.
fn decode(image: &Rgba) -> Result<String, String> {
    let luma = |x: usize, y: usize| {
        let p = (y * image.width + x) * 4;
        let (r, g, b, a) = (image.pixels[p] as u32, image.pixels[p + 1] as u32, image.pixels[p + 2] as u32, image.pixels[p + 3] as u32);
        // Treat transparent pixels as white.
        let l = (r * 299 + g * 587 + b * 114) / 1000;
        (l * a / 255 + (255 - a)) as u8
    };
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(image.width, image.height, luma);
    let grids = prepared.detect_grids();
    let grid = grids.first().ok_or("No QR code found in the image")?;
    grid.decode().map(|(_, content)| content).map_err(|e| format!("QR code found but unreadable: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let text = "https://example.com/?q=devutils&n=42";
        let image = encode(text).unwrap();
        assert_eq!(image.width, image.height);
        assert_eq!(decode(&image).unwrap(), text);
        let blank = Rgba { width: 50, height: 50, pixels: vec![255; 50 * 50 * 4] };
        assert!(decode(&blank).is_err());
    }
}
