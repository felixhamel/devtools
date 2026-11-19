//! Base64 Image: image file → data URI, and Base64 / data URI → preview.
use base64::{engine::general_purpose::STANDARD, Engine};
use eframe::egui;
use super::{encoding::decode_base64, Category, Tool};
use crate::ui::{self, icon};

#[derive(Default)]
pub struct Base64Image { text: String, path: String, info: String, error: String, texture: Option<egui::TextureHandle>, live: ui::Debounce }

impl Base64Image {
    /// Decodes `bytes` as an image and uploads it as the preview texture.
    fn show(&mut self, ctx: &egui::Context, bytes: &[u8]) {
        match load(bytes) {
            Ok((info, image)) => { self.info = info; self.error.clear(); self.texture = Some(ctx.load_texture("base64-image", image, Default::default())); }
            Err(e) => { self.info.clear(); self.error = e; self.texture = None; }
        }
    }

    /// Decodes the Base64 text field into the preview (an empty field clears it).
    fn decode_text(&mut self, ctx: &egui::Context) {
        if self.text.trim().is_empty() { self.info.clear(); self.error.clear(); self.texture = None; return; }
        match decode(&self.text) { Ok(bytes) => self.show(ctx, &bytes), Err(e) => { self.info.clear(); self.error = e; self.texture = None; } }
    }

    /// Encodes image bytes as a data URI and previews them.
    fn encode_bytes(&mut self, ctx: &egui::Context, bytes: &[u8]) {
        match data_uri(bytes) { Ok(uri) => { self.text = uri; self.show(ctx, bytes); } Err(e) => { self.error = e; self.texture = None; } }
    }

    fn encode_file(&mut self, ctx: &egui::Context, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => { self.path = path.display().to_string(); self.encode_bytes(ctx, &bytes); }
            Err(e) => { self.error = format!("Cannot read {}: {e}", path.display()); self.texture = None; }
        }
    }
}

impl Tool for Base64Image {
    fn name(&self) -> &'static str { "Base64 Image Encode/Decode" }
    fn icon(&self) -> &'static str { icon::IMAGE }
    fn category(&self) -> Category { Category::Encoding }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let area = ui.available_rect_before_wrap();
        // Files dropped anywhere on the window while this tool is open.
        for file in ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(path) = &file.path { self.encode_file(&ctx, path); }
            else if let Some(bytes) = &file.bytes { self.encode_bytes(&ctx, bytes); }
        }
        ui::description(ui, "Drop an image on the window or enter its path to encode it, or paste Base64 / a data URI to decode it. PNG, JPEG, WebP and BMP.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            ui::toolbar(left, "Input:", |ui| {
                if ui::paste_button(ui, &mut self.text, &mut self.error) { self.live.touch(ui.ctx()); }
                if ui::clear_button(ui) { *self = Self::default(); }
                ui::copy_button(ui, &self.text);
                ui.separator();
                if ui.add(ui::action_button("Decode", true)).clicked() { self.decode_text(&ctx); }
            });
            left.horizontal(|ui| {
                let encode = ui.add(ui::icon_button(icon::FOLDER_OPEN, "Encode file", false)).clicked();
                let field = ui::editor_frame().inner_margin(5).show(ui, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.path).frame(false).font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY).hint_text("/path/to/image.png"))
                }).inner;
                if encode || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    let path = std::path::PathBuf::from(self.path.trim());
                    self.encode_file(&ctx, &path);
                }
            });
            left.add_space(6.0);
            if ui::fill_editor(left, "base64", &mut self.text, "data:image/png;base64,iVBORw0KGgo…").changed() { self.live.touch(left.ctx()); }
            // Decode as the Base64 is pasted or typed.
            if self.live.due(left.ctx(), self.text.len()) { self.decode_text(&ctx); }
            ui::toolbar(right, "Preview:", |ui| { ui.label(egui::RichText::new(&self.info).color(ui::MUTED)); });
            ui::error_label(right, &self.error);
            ui::preview_frame(right, |ui| {
                if let Some(texture) = &self.texture {
                    ui.add(egui::Image::new(texture).max_width(ui.available_width()).max_height(ui.available_height()));
                }
            });
        });
        ui::drop_hint(&ctx, area, "Drop an image to encode it");
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
    let info = format!("{format:?} · {w}×{h} px · {} bytes", bytes.len());
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
