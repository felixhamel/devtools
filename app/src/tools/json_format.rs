//! JSON Format/Validate: pretty-print or minify with indentation and key-sorting
//! options, a collapsible tree view, and errors that point at the exact spot.
use eframe::egui;
use serde::Serialize;
use serde_json::Value;
use super::{Category, Tool};
use crate::ui::{self, icon, theme, Highlight, TextIo};

const SAMPLE: &str = r#"{"name":"DevUtils","version":2,"tags":["json","base64"],"settings":{"theme":"dark","fontSize":13,"beta":false},"owner":null}"#;
/// Arrays longer than this show only their first items in the tree.
const TREE_ARRAY_LIMIT: usize = 500;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Indent { Two, Four, Tab }

impl Indent {
    const ALL: [Indent; 3] = [Indent::Two, Indent::Four, Indent::Tab];
    fn label(self) -> &'static str { match self { Indent::Two => "2 spaces", Indent::Four => "4 spaces", Indent::Tab => "Tab" } }
    fn bytes(self) -> &'static [u8] { match self { Indent::Two => b"  ", Indent::Four => b"    ", Indent::Tab => b"\t" } }
}

#[derive(Clone, Copy, PartialEq)]
enum View { Text, Tree }

/// A parse error with its 1-based position, used for the caret snippet.
#[derive(Debug, PartialEq)]
struct JsonError { line: usize, column: usize, message: String }

impl JsonError {
    fn summary(&self) -> String { format!("Line {}, column {}: {}", self.line, self.column, self.message) }
}

fn parse(input: &str) -> Result<Value, JsonError> {
    serde_json::from_str(input).map_err(|e| {
        // serde_json appends " at line L column C"; the position is shown separately.
        let full = e.to_string();
        let message = full.rsplit_once(" at line ").map_or(full.clone(), |(message, _)| message.to_owned());
        JsonError { line: e.line(), column: e.column(), message }
    })
}

/// Orders object keys alphabetically, at every depth.
fn sort_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().map(|(k, v)| (k, sort_keys(v))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_keys).collect()),
        other => other,
    }
}

fn pretty(value: &Value, indent: Indent) -> String {
    let mut out = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, serde_json::ser::PrettyFormatter::with_indent(indent.bytes()));
    value.serialize(&mut serializer).expect("serializing a JSON value to memory cannot fail");
    String::from_utf8(out).expect("serde_json writes UTF-8")
}

/// The line containing the error and a `^` under the column (long lines are cut around it).
fn caret_snippet(input: &str, error: &JsonError) -> Option<String> {
    let line: Vec<char> = input.lines().nth(error.line.checked_sub(1)?)?.chars().collect();
    let column = error.column.saturating_sub(1).min(line.len());
    let start = column.saturating_sub(30);
    let end = (column + 30).min(line.len());
    let prefix = if start > 0 { "…" } else { "" };
    let excerpt: String = line[start..end].iter().collect();
    let caret = " ".repeat(prefix.chars().count() + column - start);
    Some(format!("{prefix}{excerpt}\n{caret}^"))
}

pub struct JsonFormat {
    io: TextIo,
    indent: Indent,
    sort_keys: bool,
    view: View,
    /// The last valid document, for the tree view.
    parsed: Option<Value>,
    snippet: Option<String>,
}

impl Default for JsonFormat {
    fn default() -> Self {
        let io = TextIo { input_syntax: Some("json"), output_syntax: Some("json"), ..TextIo::default() };
        Self { io, indent: Indent::Two, sort_keys: false, view: View::Text, parsed: None, snippet: None }
    }
}

impl JsonFormat {
    /// Validates the input and formats it according to the mode and options.
    fn run(&mut self) {
        self.snippet = None;
        if self.io.input.trim().is_empty() {
            self.io.output.clear();
            self.io.error.clear();
            self.parsed = None;
            return;
        }
        match parse(&self.io.input) {
            Ok(value) => {
                let value = if self.sort_keys { sort_keys(value) } else { value };
                self.io.output = if self.io.active == 1 { value.to_string() } else { pretty(&value, self.indent) };
                self.io.error.clear();
                self.parsed = Some(value);
            }
            Err(error) => {
                self.io.output.clear();
                self.io.error = error.summary();
                self.snippet = caret_snippet(&self.io.input, &error);
                self.parsed = None;
            }
        }
    }
}

impl Tool for JsonFormat {
    fn name(&self) -> &'static str { "JSON Format/Validate" }
    fn icon(&self) -> &'static str { icon::BRACKETS_CURLY }
    fn category(&self) -> Category { Category::Data }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Validates JSON as you type, then formats or minifies it. Key order is kept unless you sort.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let (indent, sort_keys) = (&mut self.indent, &mut self.sort_keys);
            let run_now = self.io.input_pane_with(left, &["Format", "Minify"], "Paste JSON here…", Some(SAMPLE), |ui| {
                let mut changed = false;
                ui.menu_button(format!("{}  Options", icon::SLIDERS_HORIZONTAL), |ui| {
                    ui.label(egui::RichText::new("Indentation").small().color(theme::SUBTLE));
                    for choice in Indent::ALL { changed |= ui.radio_value(indent, choice, choice.label()).changed(); }
                    ui.separator();
                    changed |= ui.checkbox(sort_keys, "Sort keys").changed();
                }).response.on_hover_text(format!("{} · {}", indent.label(), if *sort_keys { "keys sorted" } else { "key order kept" }));
                changed
            });
            // The options live in the output toolbar: under the input, the editor takes all the space.
            let reuse = ui::toolbar(right, "Output:", |ui| {
                ui::copy_button(ui, &self.io.output);
                let reuse = super::reuse_menu(ui, &self.io.output, "JSON Format/Validate");
                ui.separator();
                let (text, tree) = (format!("{}  Text", icon::TEXT_ALIGN_LEFT), format!("{}  Tree", icon::TREE_STRUCTURE));
                ui::segmented(ui, &mut self.view, &[(View::Text, text.as_str()), (View::Tree, tree.as_str())]);
                reuse
            });
            if run_now { self.run(); }
            if reuse { self.io.input = std::mem::take(&mut self.io.output); self.run(); }
            ui::error_label(right, &self.io.error);
            if let Some(snippet) = &self.snippet {
                ui::editor_frame().show(right, |ui| {
                    ui.set_min_width(ui.available_width());
                    // No wrapping, so the caret stays under the right column.
                    egui::ScrollArea::horizontal().id_salt("json-error").show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(snippet).monospace().color(theme::ERROR_TEXT)).extend());
                    });
                });
                right.add_space(6.0);
            }
            match self.view {
                View::Text => { ui::code_editor(right, "output", &mut self.io.output, "", Highlight::syntax(self.io.output_syntax)); }
                View::Tree => ui::preview_frame(right, |ui| match &self.parsed {
                    Some(value) => tree(ui, None, value, egui::Id::new("json-tree"), 0),
                    None => { ui.label(egui::RichText::new("Valid JSON will appear here as a tree.").color(theme::SUBTLE)); }
                }),
            }
        });
    }

    fn load_sample(&mut self) {
        self.io.input = SAMPLE.into();
        self.io.active = 0;
        self.run();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.io.input = text.to_owned();
        self.run();
    }
}

/// Draws a JSON value as a collapsible tree; objects and arrays show their size.
fn tree(ui: &mut egui::Ui, key: Option<&str>, value: &Value, id: egui::Id, depth: usize) {
    let key_text = |ui: &mut egui::Ui| if let Some(key) = key { ui.label(egui::RichText::new(format!("{key}:")).monospace().color(theme::MUTED)); };
    match value {
        Value::Object(map) => {
            let title = format!("{}{{ {} }}", key.map_or(String::new(), |k| format!("{k}: ")), map.len());
            egui::CollapsingHeader::new(egui::RichText::new(title).monospace()).id_salt(id).default_open(depth < 2).show(ui, |ui| {
                for (k, v) in map { tree(ui, Some(k), v, id.with(k), depth + 1); }
            });
        }
        Value::Array(items) => {
            let title = format!("{}[ {} ]", key.map_or(String::new(), |k| format!("{k}: ")), items.len());
            egui::CollapsingHeader::new(egui::RichText::new(title).monospace()).id_salt(id).default_open(depth < 2).show(ui, |ui| {
                for (i, v) in items.iter().take(TREE_ARRAY_LIMIT).enumerate() { tree(ui, Some(&i.to_string()), v, id.with(i), depth + 1); }
                if items.len() > TREE_ARRAY_LIMIT {
                    ui.label(egui::RichText::new(format!("… {} more items", items.len() - TREE_ARRAY_LIMIT)).color(theme::SUBTLE));
                }
            });
        }
        leaf => {
            ui.horizontal(|ui| {
                key_text(ui);
                let (text, color) = match leaf {
                    Value::String(s) => (format!("{s:?}"), egui::Color32::from_rgb(195, 232, 141)),
                    Value::Number(n) => (n.to_string(), egui::Color32::from_rgb(247, 140, 108)),
                    Value::Bool(b) => (b.to_string(), egui::Color32::from_rgb(199, 146, 234)),
                    _ => ("null".to_owned(), theme::SUBTLE),
                };
                ui.add(egui::Label::new(egui::RichText::new(text).monospace().color(color)).truncate().selectable(true));
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_with_indent_and_keeps_key_order() {
        let value = parse(r#"{"b":1,"a":{"z":[1,2],"y":true}}"#).unwrap();
        assert_eq!(value.to_string(), r#"{"b":1,"a":{"z":[1,2],"y":true}}"#);
        assert_eq!(pretty(&value, Indent::Four), "{\n    \"b\": 1,\n    \"a\": {\n        \"z\": [\n            1,\n            2\n        ],\n        \"y\": true\n    }\n}");
        assert!(pretty(&value, Indent::Tab).starts_with("{\n\t\"b\": 1,"));
    }

    #[test]
    fn sorts_keys_at_every_depth() {
        let sorted = sort_keys(parse(r#"{"b":1,"a":{"z":2,"y":[{"d":0,"c":0}]}}"#).unwrap());
        assert_eq!(sorted.to_string(), r#"{"a":{"y":[{"c":0,"d":0}],"z":2},"b":1}"#);
    }

    #[test]
    fn errors_point_at_the_position() {
        let input = "{\n  \"a\": 1\n  \"b\": 2\n}";
        let error = parse(input).unwrap_err();
        assert_eq!((error.line, error.column), (3, 3));
        assert_eq!(error.summary(), "Line 3, column 3: expected `,` or `}`");
        assert_eq!(caret_snippet(input, &error).unwrap(), "  \"b\": 2\n  ^");
    }
}
