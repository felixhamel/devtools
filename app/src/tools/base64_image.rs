use base64::{engine::general_purpose::STANDARD, Engine};
use eframe::egui;
use super::{Category, Tool};
use crate::ui;

#[derive(Default)]
pub struct Base64Image { text: String, path: String, info: String, error: String, texture: Option<egui::TextureHandle> }

impl Base64Image {
    fn show(&mut self, ctx: &egui::Context, bytes: &[u8]) {
        match load(bytes) {
            Ok((info, image)) => { self.info = info; self.error.clear(); self.texture = Some(ctx.load_texture("base64-image", image, Default::default())); }
            Err(e) => { self.info.clear(); self.error = e; self.texture = None; }
        }
    }

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
    fn badge(&self) -> &'static str { "IM" }
    fn category(&self) -> Category { Category::Encoding }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        for file in ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(path) = &file.path { self.encode_file(&ctx, path); }
            else if let Some(bytes) = &file.bytes { self.encode_bytes(&ctx, bytes); }
        }
        ui::description(ui, "Drop an image on the window or enter its path to encode it, or paste Base64 / a data URI to decode it. PNG, JPEG, WebP and BMP.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            ui::toolbar(left, "Input:", |ui| {
                if ui.add(ui::action_button("Clipboard", false)).clicked() {
                    match ui::clipboard_text() { Ok(text) => self.text = text, Err(e) => self.error = e }
                }
                if ui.add(ui::action_button("Clear", false)).clicked() { *self = Self::default(); }
                if ui.add(ui::action_button("Copy", false)).clicked() { ctx.copy_text(self.text.clone()); }
                ui.separator();
                if ui.add(ui::action_button("Decode", true)).clicked() {
                    match decode(&self.text) { Ok(bytes) => self.show(&ctx, &bytes), Err(e) => { self.info.clear(); self.error = e; self.texture = None; } }
                }
            });
            left.horizontal(|ui| {
                let encode = ui.add(ui::action_button("Encode file", false)).clicked();
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
            ui::fill_editor(left, "base64", &mut self.text, "data:image/png;base64,iVBORw0KGgo…");
            ui::toolbar(right, "Preview:", |ui| { ui.label(egui::RichText::new(&self.info).color(ui::MUTED)); });
            ui::error_label(right, &self.error);
            ui::editor_frame().show(right, |ui| {
                ui.set_min_size(ui.available_size());
                if let Some(texture) = &self.texture {
                    ui.add(egui::Image::new(texture).max_width(ui.available_width()).max_height(ui.available_height()));
                }
            });
        });
    }
}

fn data_uri(bytes: &[u8]) -> Result<String, String> {
    let format = image::guess_format(bytes).map_err(|_| "Unsupported or unrecognized image format".to_string())?;
    Ok(format!("data:{};base64,{}", format.to_mime_type(), STANDARD.encode(bytes)))
}

fn decode(text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim();
    let payload = match text.strip_prefix("data:") {
        Some(rest) => rest.split_once(',').map(|(_, data)| data).ok_or("Data URI has no comma before the Base64 data")?,
        None => text,
    };
    let compact: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD.decode(compact).map_err(|e| format!("Invalid Base64: {e}"))
}

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
