//! Regular expression tester: live matches highlighted in the text, capture
//! groups, search-and-replace preview and a menu of common patterns (Rust
//! `regex` syntax).
use std::ops::Range;
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon, Highlight};

/// The "Sample" button's pattern: email addresses, with named groups.
const SAMPLE_PATTERN: &str = r"(?P<user>[\w.+-]+)@(?P<domain>[\w-]+\.[\w.]+)";
/// The sample test string: two addresses, and one that does not match.
const SAMPLE_TEXT: &str = "Contact ada@example.com or grace.hopper@navy.mil for access.\nNot an address: someone@localhost";
/// The sample replacement, using the named groups.
const SAMPLE_REPLACEMENT: &str = "${user} at ${domain}";

/// Common patterns: name, regex and an example it matches (loaded when the test string is empty).
const PATTERNS: [(&str, &str, &str); 10] = [
    ("Email address", r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+", "Write to ada@example.com or ops+alerts@my-company.io"),
    ("URL", r"https?://[^\s/$.?#][^\s]*", "See https://example.com/docs?page=2 and http://localhost:8080/health"),
    ("IPv4 address", r"\b(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\b", "Gateway 192.168.1.1, DNS 8.8.8.8, invalid 300.1.1.1"),
    ("UUID", r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b", "id=3f2b8c1e-9a4d-4e2f-b6a1-0c5d7e8f9a0b"),
    ("ISO 8601 date/time", r"\b\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?)?\b", "Created 2024-05-17T14:30:00Z, due 2024-06-01"),
    ("Semantic version", r"\bv?\d+\.\d+\.\d+(?:-[\w.]+)?(?:\+[\w.]+)?\b", "Upgrade from v1.2.3 to 2.0.0-beta.1"),
    ("Hex color", r"#(?:[0-9a-fA-F]{3}){1,2}\b", "Colors: #56D28F, #fff and #1e1e1e"),
    ("Number", r"-?\d+(?:\.\d+)?", "Totals: 42, -7 and 3.14"),
    ("HTML tag", r"</?[a-zA-Z][^>]*>", "<p class=\"note\">Hello <b>there</b></p>"),
    ("Trailing whitespace", r"(?m)[ \t]+$", "line with spaces   \nclean line\ntab at the end\t"),
];

/// Regex options, the same as the inline flags `i`, `m` and `s`.
#[derive(Default, Clone, Copy)]
struct Flags {
    ignore_case: bool,
    /// `^` and `$` also match at line breaks.
    multi_line: bool,
    /// `.` also matches `\n`.
    dot_all: bool,
}

/// The tool: pattern, flags, replacement and test string on the left; the
/// replacement result and the match report on the right.
#[derive(Default)]
pub struct RegexTester {
    pattern: String,
    /// The test string.
    text: String,
    flags: Flags,
    /// Replacement for the "Replace" preview (`$1`, `${name}`); empty = no preview.
    replacement: String,
    /// The match report, see [`find_matches`].
    output: String,
    /// `text` after replacement; empty without a replacement.
    replaced: String,
    error: String,
    /// Byte ranges of the matches in `text`, drawn highlighted.
    matches: Vec<Range<usize>>,
}

impl RegexTester {
    /// Re-runs the pattern on the test string: report, highlighted ranges and replacement.
    fn update(&mut self) {
        let result = find_matches(&self.pattern, &self.text, self.flags)
            .and_then(|found| Ok((found, replace_all(&self.pattern, &self.text, self.flags, &self.replacement)?)));
        match result {
            Ok(((summary, ranges), replaced)) => { self.output = summary; self.matches = ranges; self.replaced = replaced; self.error.clear(); }
            Err(e) => { self.output.clear(); self.replaced.clear(); self.matches.clear(); self.error = e; }
        }
    }

    /// The common-patterns menu; picking one also loads its example when the
    /// test string is empty. Returns true when a pattern was picked.
    fn patterns_menu(&mut self, ui: &mut egui::Ui) -> bool {
        let mut picked = false;
        ui.menu_button(format!("{}  Patterns", icon::LIST_MAGNIFYING_GLASS), |ui| {
            for (name, pattern, example) in PATTERNS {
                if ui.button(name).on_hover_text(egui::RichText::new(pattern).monospace()).clicked() {
                    self.pattern = pattern.into();
                    if self.text.trim().is_empty() { self.text = example.into(); }
                    picked = true;
                    ui.close_menu();
                }
            }
        });
        picked
    }
}

impl Tool for RegexTester {
    fn name(&self) -> &'static str { "RegExp Tester" }
    fn icon(&self) -> &'static str { icon::ASTERISK }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // The ⇄ menu leaves out the tool it is drawn in.
        let tool_name = self.name();
        ui::description(ui, "Matches update as you type. Rust regex syntax (no look-around or backreferences); replacements use $1 or ${name}.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "Pattern:", |ui| {
                changed |= self.patterns_menu(ui);
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
            ui::toolbar(left, "Replace with:", |_| {});
            changed |= ui::editor_frame().show(left, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.replacement).frame(false).font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY).hint_text("Optional, e.g. $1 or ${user}"))
            }).inner.changed();
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
            ui::error_label(right, &self.error);
            if !self.replacement.is_empty() {
                // With a replacement, its result takes the top of the right side.
                let height = (right.available_height() * 0.45).max(100.0);
                let reuse = right.allocate_ui(egui::vec2(right.available_width(), height), |ui| {
                    let reuse = ui::toolbar(ui, "Result:", |ui| {
                        ui::copy_button(ui, &self.replaced);
                        super::reuse_menu(ui, &self.replaced, tool_name)
                    });
                    ui::fill_editor(ui, "replaced", &mut self.replaced, "");
                    reuse
                }).inner;
                if reuse { self.text = std::mem::take(&mut self.replaced); self.update(); }
                right.add_space(8.0);
            }
            ui::toolbar(right, "Matches:", |ui| ui::copy_button(ui, &self.output));
            ui::properties(right, "matches", &self.output);
        });
    }

    fn load_sample(&mut self) {
        self.pattern = SAMPLE_PATTERN.into();
        self.text = SAMPLE_TEXT.into();
        self.replacement = SAMPLE_REPLACEMENT.into();
        self.update();
    }

    fn accepts_input(&self) -> bool { true }

    /// Received text becomes the test string.
    fn set_input(&mut self, text: &str) {
        self.text = text.to_owned();
        self.update();
    }
}

/// Builds the regex with `flags`. The error is the `regex` crate's message,
/// which points at the problem in the pattern.
fn compile(pattern: &str, flags: Flags) -> Result<regex::Regex, String> {
    regex::RegexBuilder::new(pattern)
        .case_insensitive(flags.ignore_case).multi_line(flags.multi_line).dot_matches_new_line(flags.dot_all)
        .build().map_err(|e| e.to_string())
}

/// `text` with every match replaced (`$1`, `${name}`, `$$` for a literal `$`).
/// Empty when there is no pattern or no replacement.
fn replace_all(pattern: &str, text: &str, flags: Flags, replacement: &str) -> Result<String, String> {
    if pattern.is_empty() || replacement.is_empty() { return Ok(String::new()); }
    Ok(compile(pattern, flags)?.replace_all(text, replacement).into_owned())
}

/// Lists every match with its byte range and capture groups (named groups show
/// their name), and returns the match ranges for highlighting.
fn find_matches(pattern: &str, text: &str, flags: Flags) -> Result<(String, Vec<Range<usize>>), String> {
    if pattern.is_empty() { return Ok((String::new(), Vec::new())); }
    let re = compile(pattern, flags)?;
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
        let replaced = replace_all(SAMPLE_PATTERN, SAMPLE_TEXT, Flags::default(), SAMPLE_REPLACEMENT).unwrap();
        assert!(replaced.starts_with("Contact ada at example.com or grace.hopper at navy.mil"), "{replaced}");
    }

    #[test]
    fn replaces_with_groups() {
        let flags = Flags::default();
        assert_eq!(replace_all(r"(\w+)@(\w+)", "a@b c@d", flags, "$2/$1").unwrap(), "b/a d/c");
        assert_eq!(replace_all(r"\d", "a1b2", flags, "$$").unwrap(), "a$b$");
        assert_eq!(replace_all(r"\d", "a1", flags, "").unwrap(), "");
    }

    #[test]
    fn common_patterns_compile_and_match_their_example() {
        for (name, pattern, example) in PATTERNS {
            let (_, ranges) = find_matches(pattern, example, Flags::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!ranges.is_empty(), "{name} does not match its example");
        }
        // The IPv4 pattern rejects out-of-range octets.
        let (_, ranges) = find_matches(PATTERNS[2].1, "300.1.1.1", Flags::default()).unwrap();
        assert_eq!(ranges, Vec::<Range<usize>>::new());
    }
}
