//! RegExp Tester: live matches, highlighted in the text, with capture groups
//! (Rust `regex` syntax).
use std::ops::Range;
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon, Highlight};

const SAMPLE_PATTERN: &str = r"(?P<user>[\w.+-]+)@(?P<domain>[\w-]+\.[\w.]+)";
const SAMPLE_TEXT: &str = "Contact ada@example.com or grace.hopper@navy.mil for access.\nNot an address: someone@localhost";

#[derive(Default, Clone, Copy)]
struct Flags { ignore_case: bool, multi_line: bool, dot_all: bool }

#[derive(Default)]
pub struct RegexTester {
    pattern: String,
    text: String,
    flags: Flags,
    output: String,
    error: String,
    /// Byte ranges of the matches in `text`, drawn highlighted.
    matches: Vec<Range<usize>>,
}

impl RegexTester {
    fn update(&mut self) {
        match find_matches(&self.pattern, &self.text, self.flags) {
            Ok((summary, ranges)) => { self.output = summary; self.matches = ranges; self.error.clear(); }
            Err(e) => { self.output.clear(); self.matches.clear(); self.error = e; }
        }
    }
}

impl Tool for RegexTester {
    fn name(&self) -> &'static str { "RegExp Tester" }
    fn icon(&self) -> &'static str { icon::ASTERISK }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Matches update as you type. Uses Rust regex syntax (no look-around or backreferences).");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "Pattern:", |ui| {
                if ui::sample_button(ui) { self.load_sample(); }
                if ui::clear_button(ui) { *self = Self::default(); }
            });
            changed |= ui::editor_frame().show(left, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.pattern).frame(false).font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY).hint_text(r"e.g. (\w+)@(\w+)\.com"))
            }).inner.changed();
            left.add_space(6.0);
            left.horizontal_wrapped(|ui| {
                changed |= ui.checkbox(&mut self.flags.ignore_case, "Ignore case (i)").changed();
                changed |= ui.checkbox(&mut self.flags.multi_line, "Multiline (m)").changed();
                changed |= ui.checkbox(&mut self.flags.dot_all, "Dot matches newline (s)").changed();
            });
            left.add_space(10.0);
            ui::toolbar(left, "Test string:", |ui| {
                changed |= ui::paste_button(ui, &mut self.text, &mut self.error);
            });
            changed |= ui::code_editor(left, "text", &mut self.text, "Text to search…", Highlight::Ranges(&self.matches)).changed();
            // Re-run only when the pattern, flags or text changed, then repaint
            // so the highlighting matches the new text right away.
            if changed {
                self.update();
                left.ctx().request_repaint();
            }
            ui::toolbar(right, "Matches:", |ui| ui::copy_button(ui, &self.output));
            ui::error_label(right, &self.error);
            ui::code_editor(right, "matches", &mut self.output, "", Highlight::KeyValue);
        });
    }

    fn load_sample(&mut self) {
        self.pattern = SAMPLE_PATTERN.into();
        self.text = SAMPLE_TEXT.into();
        self.update();
    }
}

/// Lists every match with its byte range and capture groups (named groups show
/// their name), and returns the match ranges for highlighting.
fn find_matches(pattern: &str, text: &str, flags: Flags) -> Result<(String, Vec<Range<usize>>), String> {
    if pattern.is_empty() { return Ok((String::new(), Vec::new())); }
    let re = regex::RegexBuilder::new(pattern)
        .case_insensitive(flags.ignore_case).multi_line(flags.multi_line).dot_matches_new_line(flags.dot_all)
        .build().map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    let mut ranges = Vec::new();
    for (n, caps) in re.captures_iter(text).enumerate() {
        let whole = caps.get(0).expect("group 0 is always the whole match");
        ranges.push(whole.range());
        lines.push(format!("Match {}: {:?} at {}..{}", n + 1, whole.as_str(), whole.start(), whole.end()));
        for (i, name) in re.capture_names().enumerate().skip(1) {
            let label = name.map_or(format!("{i}"), |n| format!("{i} ({n})"));
            lines.push(format!("  Group {label}: {}", caps.get(i).map_or("(no match)".into(), |m| format!("{:?}", m.as_str()))));
        }
    }
    let summary = match ranges.len() {
        0 => "No matches".to_string(),
        1 => format!("1 match\n\n{}", lines.join("\n")),
        n => format!("{n} matches\n\n{}", lines.join("\n")),
    };
    Ok((summary, ranges))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_matches_and_groups() {
        let (out, ranges) = find_matches(r"(?P<user>\w+)@(\w+)", "a@b, C@d", Flags::default()).unwrap();
        assert!(out.starts_with("2 matches"));
        assert_eq!(ranges, [0..3, 5..8]);
        assert!(out.contains("Match 1: \"a@b\" at 0..3"));
        assert!(out.contains("Group 1 (user): \"C\""));
        assert!(out.contains("Group 2: \"d\""));
        assert_eq!(find_matches("x", "abc", Flags::default()).unwrap().0, "No matches");
        let ignore_case = Flags { ignore_case: true, ..Flags::default() };
        assert!(find_matches("ABC", "abc", ignore_case).unwrap().0.starts_with("1 match\n"));
        assert!(find_matches("(", "abc", Flags::default()).is_err());
    }

    #[test]
    fn sample_matches_two_addresses() {
        assert_eq!(find_matches(SAMPLE_PATTERN, SAMPLE_TEXT, Flags::default()).unwrap().1.len(), 2);
    }
}
