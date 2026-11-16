//! Line Sort/Dedupe: sort, deduplicate and trim a list, one entry per line.
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon, theme};

const SAMPLE: &str = "banana\nApple\ncherry\napple\n  banana  \ndate\nCherry";

#[derive(Default)]
pub struct Lines { input: String, output: String, error: String, sort_desc: bool, dedupe: bool, ignore_case: bool, trim_lines: bool }

/// The options that shape the result, independent of the UI.
#[derive(Clone, Copy, Default)]
struct Options { sort: bool, unique: bool, descending: bool, ignore_case: bool, trim: bool }

/// Deduplication keeps the first occurrence; sorting is stable, so equal lines
/// (e.g. differing only in case with `ignore_case`) keep their input order.
fn process(input: &str, o: Options) -> String {
    let mut lines: Vec<&str> = input.lines().map(|l| if o.trim { l.trim() } else { l }).collect();
    let key = |line: &str| if o.ignore_case { line.to_lowercase() } else { line.to_owned() };
    if o.unique {
        let mut seen = std::collections::HashSet::new();
        lines.retain(|line| seen.insert(key(line)));
    }
    if o.sort {
        // Compute each sort key once instead of at every comparison.
        if o.ignore_case { lines.sort_by_cached_key(|line| line.to_lowercase()) } else { lines.sort() }
        if o.descending { lines.reverse(); }
    }
    lines.join("\n")
}

impl Tool for Lines {
    fn name(&self) -> &'static str { "Line Sort/Dedupe" }
    fn icon(&self) -> &'static str { icon::SORT_ASCENDING }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Clean up and organize a list, one entry per line.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let clicked = ui::toolbar(left, "Input:", |ui| {
                ui::paste_button(ui, &mut self.input, &mut self.error);
                let mut clicked = None;
                if ui::sample_button(ui) { self.load_sample(); }
                if ui::clear_button(ui) { self.input.clear(); self.output.clear(); self.error.clear(); }
                ui.separator();
                for (label, sort, unique) in [("Sort + unique", true, true), ("Sort", true, false), ("Unique", false, true)] {
                    if ui.add(ui::action_button(label, sort && unique)).clicked() { clicked = Some((sort, unique)); }
                }
                clicked
            });
            left.horizontal_wrapped(|ui| {
                ui.visuals_mut().selection.bg_fill = theme::ACCENT_DARK;
                ui.checkbox(&mut self.ignore_case, "Ignore case");
                ui.checkbox(&mut self.trim_lines, "Trim whitespace");
                ui.checkbox(&mut self.dedupe, "Remove duplicates");
                ui.checkbox(&mut self.sort_desc, "Descending");
            });
            left.add_space(6.0);
            if let Some((sort, unique)) = clicked { self.run(sort, unique); }
            ui::error_label(left, &self.error);
            ui::fill_editor(left, "input", &mut self.input, "Paste your list here…");
            ui::toolbar(right, "Output:", |ui| ui::copy_button(ui, &self.output));
            ui::fill_editor(right, "output", &mut self.output, "");
        });
    }

    /// The sample has stray spaces and mixed case, so it turns on both clean-ups.
    fn load_sample(&mut self) {
        self.input = SAMPLE.into();
        self.ignore_case = true;
        self.trim_lines = true;
        self.run(true, true);
    }
}

impl Lines {
    /// Applies an action button ("Sort", "Unique"…) with the current checkboxes.
    fn run(&mut self, sort: bool, unique: bool) {
        let options = Options { sort, unique: unique || self.dedupe, descending: self.sort_desc, ignore_case: self.ignore_case, trim: self.trim_lines };
        self.output = process(&self.input, options);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_and_dedupes() {
        let input = "b\n a\nB\na\nc";
        let opts = Options { sort: true, unique: true, ..Default::default() };
        assert_eq!(process(input, opts), " a\nB\na\nb\nc");
        assert_eq!(process(input, Options { trim: true, ignore_case: true, ..opts }), "a\nb\nc");
        assert_eq!(process(input, Options { trim: true, descending: true, ..opts }), "c\nb\na\nB");
        assert_eq!(process("x\ny\nx", Options { unique: true, ..Default::default() }), "x\ny");
    }
}
