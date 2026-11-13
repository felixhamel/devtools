use eframe::egui;
use super::{Category, Tool};
use crate::ui;

#[derive(Default)]
pub struct Diff { left: String, right: String, output: String, error: String }

impl Tool for Diff {
    fn name(&self) -> &'static str { "Text Diff Checker" }
    fn badge(&self) -> &'static str { "DI" }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Compare two texts line by line. Removed lines start with − and added lines with +.");
        ui.add_space(10.0);
        let height = (ui.available_height() * 0.55).max(120.0);
        ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| {
            ui::split(ui, |left, right| {
                ui::toolbar(left, "Original:", |ui| {
                    if ui.add(ui::action_button("Clipboard", false)).clicked() { if let Ok(text) = ui::clipboard_text() { self.left = text; } }
                });
                ui::fill_editor(left, "original", &mut self.left, "Original text…");
                ui::toolbar(right, "Modified:", |ui| {
                    if ui.add(ui::action_button("Clipboard", false)).clicked() { if let Ok(text) = ui::clipboard_text() { self.right = text; } }
                });
                ui::fill_editor(right, "modified", &mut self.right, "Modified text…");
            });
        });
        ui.add_space(10.0);
        ui::toolbar(ui, "Differences:", |ui| {
            if ui.add(ui::action_button("Compare", true)).clicked() {
                match diff_lines(&self.left, &self.right) { Ok(result) => { self.output = result; self.error.clear(); } Err(error) => { self.output.clear(); self.error = error; } }
            }
            if ui.add(ui::action_button("Clear", false)).clicked() { *self = Self::default(); }
            if ui.add(ui::action_button("Copy", false)).clicked() { ui.ctx().copy_text(self.output.clone()); }
        });
        ui::error_label(ui, &self.error);
        ui::fill_editor(ui, "differences", &mut self.output, "");
    }
}

pub fn diff_lines(left: &str, right: &str) -> Result<String, String> {
    let a: Vec<_> = left.lines().collect();
    let b: Vec<_> = right.lines().collect();
    if (a.len() + 1).saturating_mul(b.len() + 1) > 1_000_000 { return Err("Text is too large to compare; use fewer lines".into()); }
    let mut lengths = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() { for j in (0..b.len()).rev() {
        lengths[i][j] = if a[i] == b[j] { lengths[i+1][j+1] + 1 } else { lengths[i+1][j].max(lengths[i][j+1]) };
    }}
    let (mut i, mut j) = (0, 0);
    let mut result = Vec::new();
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] { result.push(format!("  {}", a[i])); i += 1; j += 1; }
        else if i < a.len() && (j == b.len() || lengths[i+1][j] >= lengths[i][j+1]) { result.push(format!("- {}", a[i])); i += 1; }
        else { result.push(format!("+ {}", b[j])); j += 1; }
    }
    Ok(result.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares_lines() {
        assert_eq!(diff_lines("one\ntwo", "one\nthree").unwrap(), "  one\n- two\n+ three");
        assert_eq!(diff_lines("", "new").unwrap(), "+ new");
    }
}
