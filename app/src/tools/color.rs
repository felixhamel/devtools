//! Color Converter: any CSS color (hex, rgb(), hsl(), hwb(), oklch(), names) →
//! every format, with a picker, a preview and WCAG contrast ratios.
use csscolorparser::Color;
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon, theme, Highlight};

const SAMPLE: &str = "#56D28F";

fn channel(value: f32) -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 }

fn hex(color: &Color) -> String {
    let [r, g, b, a] = [color.r, color.g, color.b, color.a].map(channel);
    if a == 255 { format!("#{r:02X}{g:02X}{b:02X}") } else { format!("#{r:02X}{g:02X}{b:02X}{a:02X}") }
}

fn to_color32(color: &Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(channel(color.r), channel(color.g), channel(color.b), channel(color.a))
}

/// WCAG relative luminance (0 = black, 1 = white).
fn luminance(color: &Color) -> f32 {
    let [r, g, b, _] = color.to_linear_rgba();
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG contrast ratio, from 1 (none) to 21 (black on white).
fn contrast(a: &Color, b: &Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn wcag_level(ratio: f32) -> &'static str {
    match ratio {
        r if r >= 7.0 => "AAA",
        r if r >= 4.5 => "AA",
        r if r >= 3.0 => "AA for large text only",
        _ => "fails",
    }
}

/// Every format of `color` plus its contrast against white and black, as "Key: value" lines.
fn describe(color: &Color) -> String {
    let [r, g, b, a] = [color.r, color.g, color.b, color.a].map(channel);
    let rgb = if a == 255 { format!("rgb({r}, {g}, {b})") } else { format!("rgba({r}, {g}, {b}, {:.2})", color.a) };
    let white = Color::new(1.0, 1.0, 1.0, 1.0);
    let black = Color::new(0.0, 0.0, 0.0, 1.0);
    let (on_white, on_black) = (contrast(color, &white), contrast(color, &black));
    let mut lines = Vec::new();
    if let Some(name) = color.name() { lines.push(format!("Name: {name}")); }
    lines.extend([
        format!("HEX: {}", hex(color)),
        format!("RGB: {rgb}"),
        format!("HSL: {}", color.to_css_hsl()),
        format!("HWB: {}", color.to_css_hwb()),
        format!("OKLCH: {}", color.to_css_oklch()),
        String::new(),
        format!("Contrast with white: {on_white:.2}:1 · {}", wcag_level(on_white)),
        format!("Contrast with black: {on_black:.2}:1 · {}", wcag_level(on_black)),
        format!("Readable text on it: {}", if on_black >= on_white { "black" } else { "white" }),
    ]);
    lines.join("\n")
}

#[derive(Default)]
pub struct ColorTool { input: String, color: Option<Color>, output: String, error: String }

impl ColorTool {
    fn update(&mut self) {
        if self.input.trim().is_empty() { self.color = None; self.output.clear(); self.error.clear(); return; }
        match csscolorparser::parse(self.input.trim()) {
            Ok(color) => { self.output = describe(&color); self.color = Some(color); self.error.clear(); }
            Err(_) => { self.color = None; self.output.clear(); self.error = "Not a CSS color: try #56D28F, rgb(86 210 143), hsl(148 57% 58%) or teal".into(); }
        }
    }

    /// A large swatch with sample text in white and in black, to judge readability.
    fn preview(&self, ui: &mut egui::Ui, color: &Color) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 120.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, to_color32(color));
        painter.rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, theme::EDITOR_STROKE), egui::StrokeKind::Inside);
        let font = egui::FontId::proportional(18.0);
        painter.text(rect.center() - egui::vec2(0.0, 18.0), egui::Align2::CENTER_CENTER, "White text  Aa", font.clone(), egui::Color32::WHITE);
        painter.text(rect.center() + egui::vec2(0.0, 18.0), egui::Align2::CENTER_CENTER, "Black text  Aa", font, egui::Color32::BLACK);
    }
}

impl Tool for ColorTool {
    fn name(&self) -> &'static str { "Color Converter" }
    fn icon(&self) -> &'static str { icon::PALETTE }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Type any CSS color (hex, rgb(), hsl(), hwb(), oklch() or a name) or pick one.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "Color:", |ui| {
                changed |= ui::paste_button(ui, &mut self.input, &mut self.error);
                if ui::sample_button(ui) { self.input = SAMPLE.into(); changed = true; }
                if ui::clear_button(ui) { self.input.clear(); changed = true; }
                ui.separator();
                // The picker edits the current color; its result is written back as hex.
                let mut picked = self.color.as_ref().map_or(egui::Color32::from_rgb(86, 210, 143), to_color32);
                if egui::color_picker::color_edit_button_srgba(ui, &mut picked, egui::color_picker::Alpha::OnlyBlend).on_hover_text("Pick a color").changed() {
                    let [r, g, b, a] = picked.to_srgba_unmultiplied();
                    self.input = hex(&Color::from_rgba8(r, g, b, a));
                    changed = true;
                }
            });
            changed |= ui::editor_frame().show(left, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.input).frame(false).font(egui::FontId::monospace(16.0))
                    .desired_width(f32::INFINITY).hint_text("#56D28F"))
            }).inner.changed();
            if changed { self.update(); }
            left.add_space(12.0);
            if let Some(color) = &self.color { self.preview(left, color); }

            let reuse = ui::toolbar(right, "Formats:", |ui| {
                ui::copy_button(ui, &self.output);
                super::reuse_menu(ui, &self.output, "Color Converter")
            });
            if reuse { self.input = self.color.as_ref().map(hex).unwrap_or_default(); self.update(); }
            ui::error_label(right, &self.error);
            ui::code_editor(right, "formats", &mut self.output, "", Highlight::KeyValue);
        });
    }

    fn load_sample(&mut self) {
        self.input = SAMPLE.into();
        self.update();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.input = text.trim().to_owned();
        self.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_formats() {
        let out = describe(&csscolorparser::parse("rgb(86 210 143)").unwrap());
        assert!(out.contains("HEX: #56D28F"), "{out}");
        assert!(out.contains("RGB: rgb(86, 210, 143)"), "{out}");
        assert!(out.contains("HSL: hsl("), "{out}");
        assert_eq!(hex(&csscolorparser::parse("teal").unwrap()), "#008080");
        assert_eq!(csscolorparser::parse("#008080").unwrap().name(), Some("teal"));
        assert_eq!(hex(&csscolorparser::parse("rgba(255, 0, 0, 0.5)").unwrap()), "#FF000080");
    }

    #[test]
    fn wcag_contrast() {
        let white = Color::new(1.0, 1.0, 1.0, 1.0);
        let black = Color::new(0.0, 0.0, 0.0, 1.0);
        assert!((contrast(&white, &black) - 21.0).abs() < 0.01);
        // #767676 is the classic lightest gray that passes AA on white (4.54:1).
        let gray = csscolorparser::parse("#767676").unwrap();
        assert!((contrast(&gray, &white) - 4.54).abs() < 0.01);
        assert_eq!(wcag_level(4.54), "AA");
        assert_eq!(wcag_level(3.2), "AA for large text only");
    }
}
