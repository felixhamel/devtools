//! JSON Format/Validate: pretty-print or minify with indentation and key-sorting
//! options, a collapsible tree view, and errors that point at the exact spot.
use eframe::egui;
use serde::Serialize;
use serde_json::Value;
use super::{Category, Tool};
use crate::ui::{self, icon, theme, Highlight, TextIo};

/// Example for the "Sample" button: a small minified document.
const SAMPLE: &str = r#"{"name":"DevUtils","version":2,"tags":["json","base64"],"settings":{"theme":"dark","fontSize":13,"beta":false},"owner":null}"#;
/// Arrays longer than this show only their first items in the tree.
const TREE_ARRAY_LIMIT: usize = 500;

/// Index of the "Minify" action in the input pane ("Format" is 0).
const MINIFY: usize = 1;

/// Characters of context shown on each side of the error in the caret snippet.
const SNIPPET_CONTEXT: usize = 30;

/// Strings in the tree view (green).
const STRING_COLOR: egui::Color32 = egui::Color32::from_rgb(195, 232, 141);
/// Numbers in the tree view (orange).
const NUMBER_COLOR: egui::Color32 = egui::Color32::from_rgb(247, 140, 108);
/// `true` and `false` in the tree view (purple).
const BOOL_COLOR: egui::Color32 = egui::Color32::from_rgb(199, 146, 234);

/// Indentation of formatted output, picked in the Options menu.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Indent { Two, Four, Tab }

impl Indent {
    /// Every choice, in menu order.
    const ALL: [Indent; 3] = [Indent::Two, Indent::Four, Indent::Tab];
    /// Name in the Options menu and its tooltip.
    fn label(self) -> &'static str { match self { Indent::Two => "2 spaces", Indent::Four => "4 spaces", Indent::Tab => "Tab" } }
    /// One level of indentation, as `serde_json`'s formatter wants it.
    fn bytes(self) -> &'static [u8] { match self { Indent::Two => b"  ", Indent::Four => b"    ", Indent::Tab => b"\t" } }
}

/// How the output pane shows a valid document.
#[derive(Clone, Copy, PartialEq)]
enum View { Text, Tree }

/// A parse error with its 1-based position, used for the caret snippet.
#[derive(Debug, PartialEq)]
struct JsonError {
    line: usize,
    /// Counted in characters (see [`char_column`]), not bytes as `serde_json` reports it.
    column: usize,
    /// `serde_json`'s message without its " at line L column C" suffix.
    message: String,
}

impl JsonError {
    /// The message for the error label: "Line 3, column 3: expected `,` or `}`".
    fn summary(&self) -> String { format!("Line {}, column {}: {}", self.line, self.column, self.message) }
}

/// Parses JSON, keeping the error position apart from the message.
fn parse(input: &str) -> Result<Value, JsonError> {
    serde_json::from_str(input).map_err(|e| {
        // serde_json appends " at line L column C"; the position is shown separately.
        let full = e.to_string();
        let message = full.rsplit_once(" at line ").map_or(full.clone(), |(message, _)| message.to_owned());
        JsonError { line: e.line(), column: char_column(input, e.line(), e.column()), message }
    })
}

/// serde_json counts columns in bytes; people and the caret count characters
/// (`é` is two bytes). Converts a 1-based byte column of `line` to characters.
fn char_column(input: &str, line: usize, byte_column: usize) -> usize {
    let Some(text) = line.checked_sub(1).and_then(|l| input.lines().nth(l)) else { return byte_column };
    let bytes = byte_column.saturating_sub(1).min(text.len());
    text.char_indices().take_while(|&(at, _)| at < bytes).count() + 1
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

/// Pretty-prints `value` with the chosen indentation (`serde_json`'s
/// `to_string_pretty` only does 2 spaces).
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
    let start = column.saturating_sub(SNIPPET_CONTEXT);
    let end = (column + SNIPPET_CONTEXT).min(line.len());
    let prefix = if start > 0 { "…" } else { "" };
    let excerpt: String = line[start..end].iter().collect();
    let caret = " ".repeat(prefix.chars().count() + column - start);
    Some(format!("{prefix}{excerpt}\n{caret}^"))
}

/// The JSON Format/Validate tool.
pub struct JsonFormat {
    /// Input and output text; `io.active` is the action, see [`MINIFY`].
    io: TextIo,
    indent: Indent,
    sort_keys: bool,
    view: View,
    /// The last valid document, for the tree view.
    parsed: Option<Value>,
    /// The error line with a caret under the error, see [`caret_snippet`].
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
                self.io.output = if self.io.active == MINIFY { value.to_string() } else { pretty(&value, self.indent) };
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
        // The ⇄ menu leaves out the tool it is drawn in.
        let tool_name = self.name();
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
                let reuse = super::reuse_menu(ui, &self.io.output, tool_name);
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
/// The first two levels start open. `id` keeps each node's open state stable.
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
                    Value::String(s) => (format!("{s:?}"), STRING_COLOR),
                    Value::Number(n) => (n.to_string(), NUMBER_COLOR),
                    Value::Bool(b) => (b.to_string(), BOOL_COLOR),
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
    fn long_lines_are_cut_around_the_error() {
        // The error is at the `x`, column 122 of one long line.
        let input = format!("[{}x]", "1, ".repeat(40));
        let snippet = caret_snippet(&input, &parse(&input).unwrap_err()).unwrap();
        let (line, caret) = snippet.split_once('\n').unwrap();
        assert!(line.starts_with('…') && line.ends_with("x]"), "{snippet}");
        assert_eq!(line.chars().nth(caret.chars().count() - 1), Some('x'), "{snippet}");
        // An error past the last line has no snippet (and no panic).
        let eof = parse("{\n").unwrap_err();
        assert_eq!(caret_snippet("{\n", &eof), None);
    }

    #[test]
    fn errors_point_at_the_position() {
        let input = "{\n  \"a\": 1\n  \"b\": 2\n}";
        let error = parse(input).unwrap_err();
        assert_eq!((error.line, error.column), (3, 3));
        assert_eq!(error.summary(), "Line 3, column 3: expected `,` or `}`");
        assert_eq!(caret_snippet(input, &error).unwrap(), "  \"b\": 2\n  ^");
        // Columns count characters, not bytes: the caret stays under the `"` of "x".
        let accented = "{\"éé\": 1 \"x\": 2}";
        let error = parse(accented).unwrap_err();
        assert_eq!(error.column, 10);
        assert_eq!(caret_snippet(accented, &error).unwrap(), format!("{accented}\n{}^", " ".repeat(9)));
    }
}
