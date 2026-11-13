//! Text Diff Checker: line-by-line comparison of two texts.
use eframe::egui;
use super::{Category, Tool};
use crate::ui;

#[derive(Default)]
pub struct Diff { left: String, right: String, output: String }

impl Tool for Diff {
    fn name(&self) -> &'static str { "Text Diff Checker" }
    fn badge(&self) -> &'static str { "DI" }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Compare two texts line by line. Removed lines start with − and added lines with +.");
        ui.add_space(10.0);
        // The two inputs share the top 55%; the differences take the rest.
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
            if ui.add(ui::action_button("Compare", true)).clicked() { self.output = diff_lines(&self.left, &self.right); }
            if ui.add(ui::action_button("Clear", false)).clicked() { *self = Self::default(); }
            if ui.add(ui::action_button("Copy", false)).clicked() { ui.ctx().copy_text(self.output.clone()); }
        });
        ui::fill_editor(ui, "differences", &mut self.output, "");
    }
}

/// Unified line diff: `  ` unchanged, `- ` removed, `+ ` added.
/// Uses Myers' algorithm (via `similar`); on very large inputs it gives up on
/// finding the minimal diff after a short deadline so the UI never freezes.
pub fn diff_lines(left: &str, right: &str) -> String {
    let diff = similar::TextDiff::configure()
        .timeout(std::time::Duration::from_millis(500))
        .diff_lines(left, right);
    diff.iter_all_changes().map(|change| {
        let prefix = match change.tag() { similar::ChangeTag::Equal => "  ", similar::ChangeTag::Delete => "- ", similar::ChangeTag::Insert => "+ " };
        format!("{prefix}{}", change.value().trim_end_matches(['\n', '\r']))
    }).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares_lines() {
        assert_eq!(diff_lines("one\ntwo", "one\nthree"), "  one\n- two\n+ three");
        assert_eq!(diff_lines("", "new"), "+ new");
        assert_eq!(diff_lines("a\r\nb\r\n", "a\r\nb\r\n"), "  a\n  b");
    }

    #[test]
    fn handles_large_inputs() {
        // 20 000 lines each: far beyond what a quadratic table could hold.
        let left: String = (0..20_000).map(|i| format!("line {i}\n")).collect();
        let right = left.replace("line 10000\n", "changed\n");
        let out = diff_lines(&left, &right);
        assert!(out.contains("- line 10000\n+ changed"), "{}", &out[..200]);
    }
}
