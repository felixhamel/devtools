use eframe::egui;
use super::{Category, Tool};
use crate::ui;

#[derive(Default)]
pub struct RegexTester { pattern: String, text: String, ignore_case: bool, multi_line: bool, dot_all: bool, output: String, error: String }

impl Tool for RegexTester {
    fn name(&self) -> &'static str { "RegExp Tester" }
    fn badge(&self) -> &'static str { ".*" }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Matches update as you type. Uses Rust regex syntax (no look-around or backreferences).");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            ui::toolbar(left, "Pattern:", |ui| {
                if ui.add(ui::action_button("Clear", false)).clicked() { *self = Self::default(); }
            });
            let mut changed = ui::editor_frame().show(left, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.pattern).frame(false).font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY).hint_text(r"e.g. (\w+)@(\w+)\.com"))
            }).inner.changed();
            left.add_space(6.0);
            left.horizontal_wrapped(|ui| {
                changed |= ui.checkbox(&mut self.ignore_case, "Ignore case (i)").changed();
                changed |= ui.checkbox(&mut self.multi_line, "Multiline (m)").changed();
                changed |= ui.checkbox(&mut self.dot_all, "Dot matches newline (s)").changed();
            });
            left.add_space(10.0);
            ui::toolbar(left, "Test string:", |ui| {
                if ui.add(ui::action_button("Clipboard", false)).clicked() {
                    if let Ok(text) = ui::clipboard_text() { self.text = text; changed = true; }
                }
            });
            changed |= ui::fill_editor(left, "text", &mut self.text, "Text to search…").changed();
            if changed {
                match find_matches(&self.pattern, &self.text, self.ignore_case, self.multi_line, self.dot_all) {
                    Ok(out) => { self.output = out; self.error.clear(); }
                    Err(e) => { self.output.clear(); self.error = e; }
                }
            }
            ui::toolbar(right, "Matches:", |ui| {
                if ui.add(ui::action_button("Copy", false)).clicked() { ui.ctx().copy_text(self.output.clone()); }
            });
            ui::error_label(right, &self.error);
            ui::fill_editor(right, "matches", &mut self.output, "");
        });
    }
}

fn find_matches(pattern: &str, text: &str, ignore_case: bool, multi_line: bool, dot_all: bool) -> Result<String, String> {
    if pattern.is_empty() { return Ok(String::new()); }
    let re = regex::RegexBuilder::new(pattern).case_insensitive(ignore_case).multi_line(multi_line).dot_matches_new_line(dot_all)
        .build().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (n, caps) in re.captures_iter(text).enumerate() {
        let whole = caps.get(0).unwrap();
        out.push(format!("Match {}: {:?} at {}..{}", n + 1, whole.as_str(), whole.start(), whole.end()));
        for (i, name) in re.capture_names().enumerate().skip(1) {
            let label = name.map_or(format!("{i}"), |n| format!("{i} ({n})"));
            out.push(format!("  Group {label}: {}", caps.get(i).map_or("(no match)".into(), |m| format!("{:?}", m.as_str()))));
        }
    }
    Ok(if out.is_empty() { "No matches".into() } else { format!("{} match(es)\n\n{}", out.iter().filter(|l| l.starts_with("Match")).count(), out.join("\n")) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_matches_and_groups() {
        let out = find_matches(r"(?P<user>\w+)@(\w+)", "a@b, C@d", false, false, false).unwrap();
        assert!(out.starts_with("2 match(es)"));
        assert!(out.contains("Match 1: \"a@b\" at 0..3"));
        assert!(out.contains("Group 1 (user): \"C\""));
        assert!(out.contains("Group 2: \"d\""));
        assert_eq!(find_matches("x", "abc", false, false, false).unwrap(), "No matches");
        assert!(find_matches("ABC", "abc", true, false, false).unwrap().starts_with("1 match"));
        assert!(find_matches("(", "abc", false, false, false).is_err());
    }
}
