//! Line Sort/Dedupe: sort, deduplicate and trim a list, one entry per line.
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon};

/// Example for the "Sample" button: duplicates that differ in case or spacing.
const SAMPLE: &str = "banana\nApple\ncherry\napple\n  banana  \ndate\nCherry";

/// The three action buttons: label, sort, keep unique lines.
const ACTIONS: [(&str, bool, bool); 3] = [("Sort + unique", true, true), ("Sort", true, false), ("Unique", false, true)];

/// The Line Sort/Dedupe tool.
#[derive(Default)]
pub struct Lines {
    input: String,
    output: String,
    error: String,
    /// Index into [`ACTIONS`]; re-applied whenever the input or an option changes.
    action: usize,
    /// Reverse the sorted order.
    sort_desc: bool,
    /// Remove duplicates whatever the action ("Remove duplicates" checkbox).
    dedupe: bool,
    /// Compare lines case-insensitively, when sorting and deduplicating.
    ignore_case: bool,
    /// Trim each line's leading and trailing whitespace, in the output too.
    trim_whitespace: bool,
    /// Runs on large lists once typing pauses.
    live: ui::Debounce,
}

/// The options that shape the result, independent of the UI.
#[derive(Clone, Copy, Default)]
struct Options {
    sort: bool,
    unique: bool,
    /// Only applies with `sort`.
    descending: bool,
    ignore_case: bool,
    trim: bool,
}

/// Applies `o` to each line of `input` and joins the result with `\n`.
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
        // Compute each sort key once instead of at every comparison. Without
        // `ignore_case`, equal lines are identical, so stability does not matter.
        if o.ignore_case { lines.sort_by_cached_key(|line| line.to_lowercase()) } else { lines.sort_unstable() }
        if o.descending { lines.reverse(); }
    }
    lines.join("\n")
}

impl Tool for Lines {
    fn name(&self) -> &'static str { "Line Sort/Dedupe" }
    fn icon(&self) -> &'static str { icon::SORT_ASCENDING }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // The ⇄ menu leaves out the tool it is drawn in.
        let tool_name = self.name();
        ui::description(ui, "Clean up and organize a list, one entry per line.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut run_now = false;
            ui::toolbar(left, "Input:", |ui| {
                if ui::paste_button(ui, &mut self.input, &mut self.error) { run_now = true; }
                if ui::sample_button(ui) { self.load_sample(); }
                if ui::clear_button(ui) { self.input.clear(); self.output.clear(); self.error.clear(); }
                ui.separator();
                let modes: Vec<(usize, &str)> = ACTIONS.iter().enumerate().map(|(i, (label, _, _))| (i, *label)).collect();
                run_now |= ui::segmented(ui, &mut self.action, &modes);
            });
            left.horizontal_wrapped(|ui| {
                for (value, label) in [(&mut self.ignore_case, "Ignore case"), (&mut self.trim_whitespace, "Trim whitespace"), (&mut self.dedupe, "Remove duplicates"), (&mut self.sort_desc, "Descending")] {
                    run_now |= ui.checkbox(value, label).changed();
                }
            });
            left.add_space(6.0);
            ui::error_label(left, &self.error);
            if ui::fill_editor(left, "input", &mut self.input, "Paste your list here…").changed() { self.live.touch(left.ctx()); }
            if run_now || self.live.due(left.ctx(), self.input.len()) { self.run(); }
            let reuse = ui::toolbar(right, "Output:", |ui| {
                ui::copy_button(ui, &self.output);
                super::reuse_menu(ui, &self.output, tool_name)
            });
            if reuse { self.input = std::mem::take(&mut self.output); self.run(); }
            ui::fill_editor(right, "output", &mut self.output, "");
        });
    }

    /// The sample has stray spaces and mixed case, so it turns on both clean-ups.
    fn load_sample(&mut self) {
        self.input = SAMPLE.into();
        self.ignore_case = true;
        self.trim_whitespace = true;
        self.action = 0;
        self.run();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.input = text.to_owned();
        self.run();
    }
}

impl Lines {
    /// Applies the selected action with the current options.
    fn run(&mut self) {
        let (_, sort, unique) = ACTIONS[self.action];
        let options = Options { sort, unique: unique || self.dedupe, descending: self.sort_desc, ignore_case: self.ignore_case, trim: self.trim_whitespace };
        self.output = process(&self.input, options);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_combine() {
        // Unique keeps the first occurrence, in input order; ignoring case keeps the first spelling.
        assert_eq!(process("b\na\nb\nc\na", Options { unique: true, ..Default::default() }), "b\na\nc");
        assert_eq!(process("Apple\napple\nAPPLE", Options { unique: true, ignore_case: true, ..Default::default() }), "Apple");
        // Descending only applies when sorting; trimming works alone.
        assert_eq!(process("b\na", Options { descending: true, ..Default::default() }), "b\na");
        assert_eq!(process(" b \n a", Options { trim: true, ..Default::default() }), "b\na");
        // Windows line breaks are split too; an empty line sorts first.
        assert_eq!(process("b\r\na\r\n\r\nc", Options { sort: true, ..Default::default() }), "\na\nb\nc");
        // Sorting without case is stable: lines equal but for case keep their order.
        assert_eq!(process("b\nB\na", Options { sort: true, ignore_case: true, ..Default::default() }), "a\nb\nB");
        assert_eq!(process("", Options { sort: true, unique: true, ..Default::default() }), "");
    }

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
