//! Markdown Preview: source on the left, rendered preview on the right.
use eframe::egui;
use super::{Category, Tool};
use crate::ui;

#[derive(Default)]
pub struct Markdown { input: String }

impl Tool for Markdown {
    fn name(&self) -> &'static str { "Markdown Preview" }
    fn badge(&self) -> &'static str { "MD" }
    fn category(&self) -> Category { Category::Code }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Write Markdown and preview the result.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            ui::toolbar(left, "Input:", |ui| {
                if ui.add(ui::action_button("Clipboard", false)).clicked() { if let Ok(text) = ui::clipboard_text() { self.input = text; } }
                if ui.add(ui::action_button("Sample", false)).clicked() {
                    self.input = "# Markdown Preview\n\nWrite **bold** or *italic* text.\n\n- First item\n- Second item\n\n> A quote\n\n```\nCode example\n```".into();
                }
                if ui.add(ui::action_button("Clear", false)).clicked() { self.input.clear(); }
            });
            ui::fill_editor(left, "input", &mut self.input, "Enter your Markdown here…");
            ui::toolbar(right, "Preview:", |ui| {
                if ui.add(ui::action_button("Copy source", false)).clicked() { ui.ctx().copy_text(self.input.clone()); }
            });
            self.preview(right);
        });
    }
}

impl Markdown {
    /// A simple line-based renderer: headings, bullets, quotes and fenced code blocks.
    fn preview(&self, ui: &mut egui::Ui) {
        ui::editor_frame().show(ui, |ui| {
            let size = ui.available_size();
            egui::ScrollArea::vertical().id_salt("preview").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_min_size(size);
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
            });
        });
    }
}
