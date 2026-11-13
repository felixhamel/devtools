use eframe::egui;
use super::{Category, Tool};
use crate::ui;

#[derive(Default)]
pub struct Lines { input: String, output: String, sort_desc: bool, dedupe: bool, ignore_case: bool, trim_lines: bool }

impl Lines {
    fn process(&self, sort: bool, unique: bool) -> String {
        let mut lines: Vec<String> = self.input.lines().map(|l| if self.trim_lines { l.trim().to_owned() } else { l.to_owned() }).collect();
        if unique || self.dedupe {
            let mut seen = std::collections::HashSet::new();
            lines.retain(|line| seen.insert(if self.ignore_case { line.to_lowercase() } else { line.clone() }));
        }
        if sort {
            lines.sort_by_key(|line| if self.ignore_case { line.to_lowercase() } else { line.clone() });
            if self.sort_desc { lines.reverse(); }
        }
        lines.join("\n")
    }
}

impl Tool for Lines {
    fn name(&self) -> &'static str { "Line Sort/Dedupe" }
    fn badge(&self) -> &'static str { "AZ" }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Clean up and organize a list, one entry per line.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let clicked = ui::toolbar(left, "Input:", |ui| {
                if ui.add(ui::action_button("Clipboard", false)).clicked() { if let Ok(text) = ui::clipboard_text() { self.input = text; } }
                if ui.add(ui::action_button("Clear", false)).clicked() { self.input.clear(); self.output.clear(); }
                ui.separator();
                let mut clicked = None;
                for (label, sort, unique) in [("Sort + unique", true, true), ("Sort", true, false), ("Unique entries", false, true)] {
                    if ui.add(ui::action_button(label, sort && unique)).clicked() { clicked = Some((sort, unique)); }
                }
                clicked
            });
            left.horizontal_wrapped(|ui| {
                ui.visuals_mut().selection.bg_fill = egui::Color32::from_rgb(76, 151, 110);
                ui.checkbox(&mut self.ignore_case, "Ignore case");
                ui.checkbox(&mut self.trim_lines, "Trim whitespace");
                ui.checkbox(&mut self.dedupe, "Remove duplicates");
                ui.checkbox(&mut self.sort_desc, "Descending");
            });
            left.add_space(6.0);
            if let Some((sort, unique)) = clicked { self.output = self.process(sort, unique); }
            ui::fill_editor(left, "input", &mut self.input, "Paste your list here…");
            ui::toolbar(right, "Output:", |ui| {
                if ui.add(ui::action_button("Copy", false)).clicked() { ui.ctx().copy_text(self.output.clone()); }
            });
            ui::fill_editor(right, "output", &mut self.output, "");
        });
    }
}
