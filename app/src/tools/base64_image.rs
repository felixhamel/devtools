//! Base64 Image: image file → data URI, and Base64 / data URI → preview.
use base64::{engine::general_purpose::STANDARD, Engine};
use eframe::egui;
use super::{encoding::decode_base64, Category, Tool};
use crate::ui::{self, icon};

/// Image types the Open dialog offers.
const EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];

#[derive(Default)]
pub struct Base64Image {
    text: String,
    /// File name of the encoded image, if it came from a file.
    name: String,
    info: String,
    error: String,
    texture: Option<egui::TextureHandle>,
    live: ui::Debounce,
}

impl Base64Image {
    /// Decodes `bytes` as an image and uploads it as the preview texture.
    fn show(&mut self, ctx: &egui::Context, bytes: &[u8]) {
        match load(bytes) {
            Ok((info, image)) => {
                self.info = if self.name.is_empty() { info } else { format!("{} · {info}", self.name) };
                self.error.clear();
                self.texture = Some(ctx.load_texture("base64-image", image, Default::default()));
            }
            Err(e) => { self.info.clear(); self.error = e; self.texture = None; }
        }
    }

    /// Decodes the Base64 text field into the preview (an empty field clears it).
    fn decode_text(&mut self, ctx: &egui::Context) {
        if self.text.trim().is_empty() { self.info.clear(); self.error.clear(); self.texture = None; return; }
        match decode(&self.text) { Ok(bytes) => self.show(ctx, &bytes), Err(e) => { self.info.clear(); self.error = e; self.texture = None; } }
    }

    /// Encodes an image file as a data URI and previews it.
    fn encode(&mut self, ctx: &egui::Context, name: String, bytes: &[u8]) {
        match data_uri(bytes) {
            Ok(uri) => { self.text = uri; self.name = name; self.show(ctx, bytes); }
            Err(e) => { self.error = format!("{name}: {e}"); self.texture = None; }
        }
    }

    fn encode_file(&mut self, ctx: &egui::Context, file: Result<(String, Vec<u8>), String>) {
        match file { Ok((name, bytes)) => self.encode(ctx, name, &bytes), Err(e) => self.error = e }
    }
}

/// The example: a small gradient disc on a transparent background, as PNG.
fn sample_png() -> Vec<u8> {
    let image = image::RgbaImage::from_fn(96, 96, |x, y| {
        let distance = ((x as f32 - 47.5).powi(2) + (y as f32 - 47.5).powi(2)).sqrt();
        let t = y as f32 / 95.0;
        // Opaque inside, a one-pixel soft edge, transparent outside.
        let alpha = ((44.0 - distance).clamp(0.0, 1.0) * 255.0) as u8;
        let mix = |a: f32, b: f32| (a * (1.0 - t) + b * t) as u8;
        image::Rgba([mix(130.0, 110.0), mix(220.0, 170.0), mix(169.0, 255.0), alpha])
    });
    let mut png = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).expect("encoding a PNG in memory cannot fail");
    png
}

impl Tool for Base64Image {
    fn name(&self) -> &'static str { "Base64 Image Encode/Decode" }
    fn icon(&self) -> &'static str { icon::IMAGE }
    fn category(&self) -> Category { Category::Encoding }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let area = ui.available_rect_before_wrap();
        // Files dropped anywhere on the window while this tool is open.
        for file in ctx.input(|i| i.raw.dropped_files.clone()) { self.encode_file(&ctx, ui::read_dropped(&file)); }
        ui::description(ui, "Drop or open an image to get its data URI, or paste Base64 / a data URI to preview it. PNG, JPEG, WebP and BMP.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            ui::toolbar(left, "Data URI:", |ui| {
                if ui::paste_button(ui, &mut self.text, &mut self.error) { self.name.clear(); self.live.touch(ui.ctx()); }
                if ui::sample_button(ui) { self.encode(&ctx, "sample.png".into(), &sample_png()); }
                if ui::clear_button(ui) { *self = Self::default(); }
                ui.separator();
                if ui.add(ui::icon_button(icon::FOLDER_OPEN, "Open image", false)).clicked() {
                    if let Some(path) = ui::pick_files("Open an image to encode", &EXTENSIONS).first() { self.encode_file(&ctx, ui::read_path(path)); }
                }
                ui::copy_button(ui, &self.text);
            });
            if ui::fill_editor(left, "base64", &mut self.text, "data:image/png;base64,iVBORw0KGgo…").changed() {
                self.name.clear();
                self.live.touch(left.ctx());
            }
            // Decode as the Base64 is pasted or typed.
            if self.live.due(left.ctx(), self.text.len()) { self.decode_text(&ctx); }
            ui::toolbar(right, "Preview:", |ui| { ui.label(egui::RichText::new(&self.info).color(ui::MUTED)); });
            ui::error_label(right, &self.error);
            ui::preview_frame(right, |ui| match &self.texture {
                Some(texture) => {
                    // Shown at its own size (shrunk to fit), centered.
                    let area = ui.available_size();
                    let size = texture.size_vec2() * (area.x / texture.size_vec2().x).min(area.y / texture.size_vec2().y).min(1.0);
                    ui::center_vertically(ui, size.y);
                    ui.vertical_centered(|ui| ui.add(egui::Image::new(texture).fit_to_exact_size(size)));
                }
                None => ui::empty_state(ui, icon::IMAGE, "Drop an image on the window, open one, or paste a data URI."),
            });
        });
        ui::drop_hint(&ctx, area, "Drop an image to encode it");
    }

    fn load_sample(&mut self) {
        // The preview needs the egui context: it is decoded on the next frame.
        if let Ok(uri) = data_uri(&sample_png()) {
            self.text = uri;
            self.name = "sample.png".into();
            self.live.trigger();
        }
    }

    fn accepts_input(&self) -> bool { true }

    /// Received Base64 or data URI is decoded on the next frame (textures need the egui context).
    fn set_input(&mut self, text: &str) {
        self.text = text.trim().to_owned();
        self.live.trigger();
    }
}

/// `data:image/png;base64,…` with the MIME type detected from the bytes.
fn data_uri(bytes: &[u8]) -> Result<String, String> {
    let format = image::guess_format(bytes).map_err(|_| "Unsupported or unrecognized image format".to_string())?;
    Ok(format!("data:{};base64,{}", format.to_mime_type(), STANDARD.encode(bytes)))
}

/// Accepts a full data URI or bare Base64.
fn decode(text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim();
    let payload = match text.strip_prefix("data:") {
        Some(rest) => rest.split_once(',').map(|(_, data)| data).ok_or("Data URI has no comma before the Base64 data")?,
        None => text,
    };
    decode_base64(payload)
}

/// Decodes an image into pixels plus a one-line summary (format, size, bytes).
fn load(bytes: &[u8]) -> Result<(String, egui::ColorImage), String> {
    let format = image::guess_format(bytes).map_err(|_| "Unsupported or unrecognized image format".to_string())?;
    let image = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = image.dimensions();
    let info = format!("{} · {w}×{h} px · {}", format!("{format:?}").to_uppercase(), ui::human_size(bytes.len()));
    Ok((info, egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], image.as_raw())))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_round_trip() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let uri = data_uri(&png).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
        let bytes = decode(&uri).unwrap();
        assert_eq!(bytes, png);
        assert!(load(&bytes).unwrap().0.contains("3×2 px"));
        assert!(decode("data:image/png;base64").is_err());
        assert!(data_uri(b"not an image").is_err());
    }
}
