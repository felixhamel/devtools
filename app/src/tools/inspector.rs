//! String Inspector: counts (visible characters, code points, bytes, UTF-16),
//! line endings, invisible or unusual characters highlighted in the text, and a
//! character-by-character table with Unicode names.
use std::ops::Range;
use eframe::egui;
use unicode_segmentation::UnicodeSegmentation;
use super::{Category, Tool};
use crate::ui::{self, icon, theme, Highlight};

/// The example: non-breaking spaces, a zero-width space, CRLF, a tab and an emoji with a skin tone.
const SAMPLE: &str = "Price:\u{a0}42\u{a0}€ — café\u{200b}\r\nTab\there 👋🏽";
/// The character table lists at most this many characters.
const TABLE_LIMIT: usize = 300;

/// Characters that are easy to miss and often cause bugs: non-breaking and
/// exotic spaces, zero-width characters, the BOM, soft hyphens, direction
/// controls ("Trojan Source"), and control characters other than tab/newline.
fn is_special(c: char) -> bool {
    matches!(c,
        '\u{a0}' | '\u{ad}' | '\u{1680}' | '\u{180e}' | '\u{2000}'..='\u{200f}' | '\u{2028}' | '\u{2029}'
        | '\u{202a}'..='\u{202f}' | '\u{205f}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{3000}' | '\u{feff}')
        || (c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
}

/// The Unicode name of `c`, e.g. "NO-BREAK SPACE"; `<control>` or `<unnamed>` when it has none.
fn char_name(c: char) -> String {
    unicode_names2::name(c).map_or_else(|| if c.is_control() { "<control>".into() } else { "<unnamed>".into() }, |n| n.to_string())
}

/// How a character is shown in the table: line breaks, tabs and spaces as symbols,
/// other invisible characters as a dot.
fn visible(c: char) -> String {
    match c {
        '\n' => "↵".into(), '\r' => "␍".into(), '\t' => "⇥".into(), ' ' => "␣".into(),
        c if is_special(c) || c.is_control() => "·".into(),
        c => c.to_string(),
    }
}

/// "LF", "CRLF", "CR", "Mixed (LF ×2, CRLF ×1)" or "None".
fn line_endings(text: &str) -> String {
    // Each CRLF also counts as one LF and one CR: subtract it from both.
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count() - crlf;
    let cr = text.matches('\r').count() - crlf;
    let found: Vec<(&str, usize)> = [("LF", lf), ("CRLF", crlf), ("CR", cr)].into_iter().filter(|(_, n)| *n > 0).collect();
    match found.as_slice() {
        [] => "None".into(),
        [(kind, _)] => (*kind).into(),
        many => format!("Mixed ({})", many.iter().map(|(k, n)| format!("{k} ×{n}")).collect::<Vec<_>>().join(", ")),
    }
}

/// The statistics as "Key: value" lines, and the byte ranges of special characters.
fn inspect(text: &str) -> (String, Vec<Range<usize>>) {
    let specials: Vec<(usize, char)> = text.char_indices().filter(|(_, c)| is_special(*c)).collect();
    let ranges = specials.iter().map(|(i, c)| *i..*i + c.len_utf8()).collect();
    let mut lines = vec![
        format!("Characters: {}", text.graphemes(true).count()),
        format!("Code points: {}", text.chars().count()),
        format!("UTF-8 bytes: {}", text.len()),
        format!("UTF-16 units: {}", text.encode_utf16().count()),
        format!("Words: {}", text.unicode_words().count()),
        format!("Lines: {}", text.lines().count()),
        format!("Line endings: {}", line_endings(text)),
        format!("Non-ASCII: {}", text.chars().filter(|c| !c.is_ascii()).count()),
        format!("Leading/trailing whitespace: {}", if text != text.trim() { "yes" } else { "no" }),
        format!("Special characters: {}", specials.len()),
    ];
    // Group the special characters: "U+00A0 NO-BREAK SPACE ×2".
    let mut kinds: Vec<(char, usize)> = Vec::new();
    for (_, c) in &specials {
        match kinds.iter_mut().find(|(k, _)| k == c) { Some((_, n)) => *n += 1, None => kinds.push((*c, 1)) }
    }
    for (c, n) in kinds { lines.push(format!("  U+{:04X} {} ×{n}", c as u32, char_name(c))); }
    (lines.join("\n"), ranges)
}

/// The String Inspector tool: the text on the left, its statistics and characters on the right.
#[derive(Default)]
pub struct StringInspector {
    input: String,
    error: String,
    /// The statistics, as `Key: value` lines.
    stats: String,
    /// Byte ranges of the special characters, highlighted in the editor.
    specials: Vec<Range<usize>>,
}

impl StringInspector {
    /// Recomputes the statistics and highlights after the input changed.
    fn update(&mut self) { (self.stats, self.specials) = inspect(&self.input); }

    /// The character table, with a note when the text is longer than [`TABLE_LIMIT`].
    fn table(&self, ui: &mut egui::Ui) {
        // Long character names scroll sideways instead of overflowing the frame.
        egui::ScrollArea::horizontal().id_salt("inspector-table").show(ui, |ui| self.grid(ui));
        let total = self.input.chars().count();
        if total > TABLE_LIMIT { ui.label(egui::RichText::new(format!("… {} more characters", total - TABLE_LIMIT)).color(theme::SUBTLE)); }
    }

    /// One row per character: index, glyph, code point, UTF-8 bytes and name.
    /// Special characters are drawn in the warning color.
    fn grid(&self, ui: &mut egui::Ui) {
        egui::Grid::new("inspector-chars").num_columns(5).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
            for header in ["#", "Char", "Code point", "UTF-8", "Name"] { ui.label(egui::RichText::new(header).small().color(theme::SUBTLE)); }
            ui.end_row();
            for (i, c) in self.input.chars().take(TABLE_LIMIT).enumerate() {
                let color = if is_special(c) { theme::WARNING } else { theme::TEXT };
                let utf8: Vec<String> = c.to_string().bytes().map(|b| format!("{b:02X}")).collect();
                ui.label(egui::RichText::new(i.to_string()).monospace().color(theme::SUBTLE));
                ui.label(egui::RichText::new(visible(c)).monospace().color(color));
                ui.label(egui::RichText::new(format!("U+{:04X}", c as u32)).monospace().color(color));
                ui.label(egui::RichText::new(utf8.join(" ")).monospace().color(theme::MUTED));
                ui.label(egui::RichText::new(char_name(c)).small().color(color));
                ui.end_row();
            }
        });
    }
}

impl Tool for StringInspector {
    fn name(&self) -> &'static str { "String Inspector" }
    fn icon(&self) -> &'static str { icon::MAGNIFYING_GLASS }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Counts, line endings and hidden characters (non-breaking or zero-width spaces, BOM, direction controls…), highlighted in the text.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "Text:", |ui| {
                changed |= ui::paste_button(ui, &mut self.input, &mut self.error);
                if ui::sample_button(ui) { self.input = SAMPLE.into(); changed = true; }
                if ui::clear_button(ui) { self.input.clear(); changed = true; }
            });
            ui::error_label(left, &self.error);
            changed |= ui::code_editor(left, "text", &mut self.input, "Paste text to inspect…", Highlight::Ranges(&self.specials)).changed();
            // The editor was just drawn with the old ranges: repaint to highlight the new ones.
            if changed { self.update(); left.ctx().request_repaint(); }

            let stats_height = (right.available_height() * 0.42).max(120.0);
            right.allocate_ui(egui::vec2(right.available_width(), stats_height), |ui| {
                ui::toolbar(ui, "Statistics:", |ui| ui::copy_button(ui, &self.stats));
                ui::properties(ui, "stats", &self.stats);
            });
            right.add_space(8.0);
            ui::toolbar(right, "Characters:", |_| {});
            ui::preview_frame(right, |ui| self.table(ui));
        });
    }

    fn load_sample(&mut self) {
        self.input = SAMPLE.into();
        self.update();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.input = text.to_owned();
        self.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_graphemes_bytes_and_utf16() {
        let (stats, _) = inspect("café 👋🏽");
        assert!(stats.contains("Characters: 6\n"), "{stats}"); // 👋🏽 is one visible character…
        assert!(stats.contains("Code points: 7\n"), "{stats}"); // …made of two code points
        assert!(stats.contains("UTF-8 bytes: 14\n"), "{stats}");
        assert!(stats.contains("UTF-16 units: 9\n"), "{stats}");
    }

    #[test]
    fn finds_hidden_characters_and_line_endings() {
        let (stats, ranges) = inspect(SAMPLE);
        assert!(stats.contains("Special characters: 3"), "{stats}");
        assert!(stats.contains("U+00A0 NO-BREAK SPACE ×2"), "{stats}");
        assert!(stats.contains("U+200B ZERO WIDTH SPACE ×1"), "{stats}");
        assert_eq!(&SAMPLE[ranges[0].clone()], "\u{a0}");
        assert_eq!(line_endings("a\nb\r\nc\n"), "Mixed (LF ×2, CRLF ×1)");
        assert_eq!(line_endings("a\r\nb"), "CRLF");
        assert_eq!(line_endings("one line"), "None");
        assert!(!is_special('\t') && is_special('\u{feff}') && is_special('\u{202e}'));
    }
}
