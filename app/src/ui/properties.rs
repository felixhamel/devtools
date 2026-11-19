//! `Key: value` reports shown as rows with a copy button on each value
//! (hashes, color formats, dates, certificate fields…).
use eframe::egui;
use super::{preview_frame, theme};
use super::widgets::{copy_with_feedback, CopyStyle};

/// Output "language" of `Key: value` reports, shown with [`properties`].
pub const KEY_VALUE: &str = "key-value";

/// A line of a `Key: value` report (see [`parse_properties`]).
#[derive(Debug, PartialEq)]
pub enum Property<'a> {
    /// `Key: value`, with the value trimmed.
    Pair {
        key: &'a str,
        value: &'a str,
        /// The line was indented: it belongs to the heading above.
        indent: bool,
    },
    /// A line without a key, e.g. an item listed under a heading.
    Text {
        value: &'a str,
        /// The line was indented: it belongs to the heading above.
        indent: bool,
    },
    /// A `Heading:` line followed by indented lines.
    Heading(&'a str),
    /// A blank line between groups.
    Gap,
}

/// Splits a report into [`Property`] lines: `Key: value` (split at the first
/// ": "), `Heading:` followed by indented lines (a single indented line
/// becomes the heading's value), other lines, and blank lines.
pub fn parse_properties(text: &str) -> Vec<Property<'_>> {
    let lines: Vec<&str> = text.lines().collect();
    let is_indented = |line: &str| line.starts_with(char::is_whitespace) && !line.trim().is_empty();
    let mut items = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        let indent = is_indented(line);
        let trimmed = line.trim_start();
        i += 1;
        if trimmed.is_empty() {
            items.push(Property::Gap);
        } else if let Some((key, value)) = trimmed.split_once(": ") {
            items.push(Property::Pair { key, value: value.trim(), indent });
        } else if let (Some(key), false) = (trimmed.strip_suffix(':'), indent) {
            let block = lines[i..].iter().take_while(|l| is_indented(l)).count();
            if block == 1 {
                items.push(Property::Pair { key, value: lines[i].trim(), indent: false });
                i += 1;
            } else {
                items.push(Property::Heading(key));
            }
        } else {
            items.push(Property::Text { value: trimmed, indent });
        }
    }
    items
}

/// Shows a `Key: value` report as rows: muted keys, bright selectable values
/// and a copy button for each value. `id` must be unique within the tool.
pub fn properties(ui: &mut egui::Ui, id: &str, text: &str) {
    property_list(ui, id, &parse_properties(text));
}

/// Draws [`Property`] rows in a scrolling frame, as [`properties`] does.
pub fn property_list(ui: &mut egui::Ui, id: &str, items: &[Property]) {
    /// Left offset of indented rows.
    const INDENT: f32 = 16.0;
    /// Room kept at the right of each row for its copy button.
    const COPY_WIDTH: f32 = 32.0;
    let key_font = egui::FontId::proportional(13.0);
    let widest = items.iter()
        .filter_map(|item| match item { Property::Pair { key, indent, .. } => Some((*key, *indent)), _ => None })
        .map(|(key, indent)| ui.fonts(|f| f.layout_no_wrap(key.to_owned(), key_font.clone(), theme::MUTED).size().x) + if indent { INDENT } else { 0.0 })
        .fold(0.0_f32, f32::max);
    // The hovered row (from the previous frame) shows its copy button fully.
    let hover_id = egui::Id::new((id, "hovered-row"));
    let hovered = ui.data(|d| d.get_temp::<Option<usize>>(hover_id)).flatten();
    let mut now_hovered = None;
    preview_frame(ui, |ui| {
        // Keys line up in a column as wide as the widest key, up to 42% of the row.
        let key_width = (widest + 14.0).min(ui.available_width() * 0.42);
        for (n, item) in items.iter().enumerate() {
            let (key, value, indent) = match *item {
                Property::Gap => { ui.add_space(8.0); continue; }
                Property::Heading(text) => {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(text).color(theme::MUTED));
                    continue;
                }
                Property::Pair { key, value, indent } => (Some(key), value, indent),
                Property::Text { value, indent } => (None, value, indent),
            };
            // Reserve a shape slot below the row: the hover background is only
            // known once the row is laid out, but must be painted under it.
            let background = ui.painter().add(egui::Shape::Noop);
            let row = ui.horizontal_top(|ui| {
                if indent { ui.add_space(INDENT); }
                if let Some(key) = key {
                    let width = (key_width - if indent { INDENT } else { 0.0 }).max(20.0);
                    ui.allocate_ui_with_layout(egui::vec2(width, 0.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.set_width(width);
                        ui.add(egui::Label::new(egui::RichText::new(key).font(key_font.clone()).color(theme::MUTED)).truncate());
                    });
                }
                let width = (ui.available_width() - COPY_WIDTH).max(40.0);
                ui.allocate_ui_with_layout(egui::vec2(width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(width);
                    ui.add(egui::Label::new(egui::RichText::new(value).monospace().color(theme::TEXT_STRONG)).wrap());
                });
                let style = if hovered == Some(n) { CopyStyle::Icon } else { CopyStyle::Faint };
                copy_with_feedback(ui, value, egui::Id::new((id, "copy", n)), style);
            });
            let rect = row.response.rect.expand2(egui::vec2(6.0, 2.0));
            if ui.rect_contains_pointer(rect) {
                now_hovered = Some(n);
                ui.painter().set(background, egui::epaint::RectShape::filled(rect, 5.0, theme::ROW_HOVER));
            }
        }
    });
    if now_hovered != hovered {
        ui.data_mut(|d| d.insert_temp(hover_id, now_hovered));
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_value_reports() {
        let report = "Read as: seconds\nUTC: Tue 14 Nov 2023, 22:13:20\n\nNext runs:\n  Mon 09:00\n  Tue 09:00\nSerial:\n  2D:F4\n2 matches\n  Group 1 (user): \"ada\"";
        assert_eq!(parse_properties(report), [
            Property::Pair { key: "Read as", value: "seconds", indent: false },
            Property::Pair { key: "UTC", value: "Tue 14 Nov 2023, 22:13:20", indent: false },
            Property::Gap,
            Property::Heading("Next runs"),
            Property::Text { value: "Mon 09:00", indent: true },
            Property::Text { value: "Tue 09:00", indent: true },
            Property::Pair { key: "Serial", value: "2D:F4", indent: false },
            Property::Text { value: "2 matches", indent: false },
            Property::Pair { key: "Group 1 (user)", value: "\"ada\"", indent: true },
        ]);
    }

    #[test]
    fn parses_report_edge_cases() {
        assert_eq!(parse_properties(""), []);
        // Split at the first ": " only; Windows line breaks and trailing spaces are ignored.
        assert_eq!(parse_properties("Error: line 2: bad  \r\nNext: 1"), [
            Property::Pair { key: "Error", value: "line 2: bad", indent: false },
            Property::Pair { key: "Next", value: "1", indent: false },
        ]);
        // A heading with nothing indented under it stays a heading.
        assert_eq!(parse_properties("Tags:\nNext: 1"), [Property::Heading("Tags"), Property::Pair { key: "Next", value: "1", indent: false }]);
        // An indented line ending with ":" is text, not a nested heading; "a:b" has no key.
        assert_eq!(parse_properties("Group:\n  Sub:\n  x\nurl:http"), [
            Property::Heading("Group"),
            Property::Text { value: "Sub:", indent: true },
            Property::Text { value: "x", indent: true },
            Property::Text { value: "url:http", indent: false },
        ]);
        // Blank lines with spaces are gaps too.
        assert_eq!(parse_properties("a: 1\n   \nb: 2")[1], Property::Gap);
    }
}
