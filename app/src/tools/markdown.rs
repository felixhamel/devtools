//! Markdown Preview: source on the left, rendered preview on the right.
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon};

const SAMPLE: &str = "# Markdown Preview\n\nWrite **bold** or *italic* text.\n\n## A list\n\n- First item\n- Second item\n\n> A quote\n\n```\nlet code = \"example\";\n```";

#[derive(Default)]
pub struct Markdown { input: String, error: String }

impl Tool for Markdown {
    fn name(&self) -> &'static str { "Markdown Preview" }
    fn icon(&self) -> &'static str { icon::MARKDOWN_LOGO }
    fn category(&self) -> Category { Category::Code }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Write Markdown and preview the result.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            ui::toolbar(left, "Input:", |ui| {
                ui::paste_button(ui, &mut self.input, &mut self.error);
                if ui::sample_button(ui) { self.load_sample(); }
                if ui::clear_button(ui) { self.input.clear(); self.error.clear(); }
            });
            ui::error_label(left, &self.error);
            ui::code_editor(left, "input", &mut self.input, "Enter your Markdown here…", ui::Highlight::Syntax("md"));
            ui::toolbar(right, "Preview:", |ui| ui::copy_button(ui, &self.input));
            ui::preview_frame(right, |ui| self.preview(ui));
        });
    }

    fn load_sample(&mut self) { self.input = SAMPLE.into(); }
}

impl Markdown {
    /// A simple line-based renderer: headings, bullets, quotes and fenced code blocks.
    fn preview(&self, ui: &mut egui::Ui) {
        let mut code = false;
        for line in self.input.lines() {
            if line.trim_start().starts_with("```") { code = !code; continue; }
            if code { ui.monospace(line); }
            else if let Some(text) = line.strip_prefix("# ") { ui.heading(text); }
            else if let Some(text) = line.strip_prefix("## ") { ui.label(egui::RichText::new(text).size(21.0).strong()); }
            else if let Some(text) = line.strip_prefix("### ") { ui.label(egui::RichText::new(text).size(17.0).strong()); }
            else if let Some(text) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) { ui.label(format!("• {text}")); }
            else if let Some(text) = line.strip_prefix("> ") { ui.label(egui::RichText::new(format!("│ {text}")).italics().color(egui::Color32::LIGHT_GRAY)); }
            else if line.is_empty() { ui.add_space(8.0); }
            else { ui.label(line); }
        }
    }
}
