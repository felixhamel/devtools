//! JSON ↔ CSV: an array of objects to CSV, where nested objects become
//! `parent.child` columns, and CSV back to JSON, nesting those columns again.
use std::collections::{HashMap, HashSet};
use eframe::egui;
use serde_json::{Map, Value};
use super::{Category, Tool};
use crate::ui::{self, icon, theme, TextIo};

/// Example for the "Sample" button: nested objects, an array, a missing key and a null.
const SAMPLE: &str = r#"[
  {"id": 1, "name": "Ada Lovelace", "active": true, "address": {"city": "London", "country": "UK"}, "tags": ["math", "poetry"]},
  {"id": 2, "name": "Grace Hopper", "active": false, "address": {"city": "New York", "country": "US"}, "tags": []},
  {"id": 3, "name": "Alan Turing, OBE", "active": true, "address": {"city": "Wilmslow", "country": null}}
]"#;

/// The two directions, in the order of the mode switch.
const MODES: [&str; 2] = ["JSON \u{2192} CSV", "CSV \u{2192} JSON"];

/// The CSV field separator.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Delimiter { Auto, Comma, Semicolon, Tab }

impl Delimiter {
    /// Every choice, in menu order.
    const ALL: [Delimiter; 4] = [Delimiter::Auto, Delimiter::Comma, Delimiter::Semicolon, Delimiter::Tab];

    /// Name in the Options menu.
    fn label(self) -> &'static str {
        match self { Delimiter::Auto => "Auto (comma when writing)", Delimiter::Comma => "Comma  ,", Delimiter::Semicolon => "Semicolon  ;", Delimiter::Tab => "Tab" }
    }

    /// The separator to read `csv` with; `Auto` guesses it (see [`guess_delimiter`]).
    fn reading(self, csv: &str) -> u8 {
        match self {
            Delimiter::Auto => guess_delimiter(csv).unwrap_or(b','),
            _ => self.writing(),
        }
    }

    /// The separator to write CSV with.
    fn writing(self) -> u8 {
        match self { Delimiter::Auto | Delimiter::Comma => b',', Delimiter::Semicolon => b';', Delimiter::Tab => b'\t' }
    }
}

/// The separator `csv` uses most on its first line (tab, semicolon or comma;
/// comma on a tie), or `None` when the first line has none of them.
pub(super) fn guess_delimiter(csv: &str) -> Option<u8> {
    let first = csv.lines().next().unwrap_or_default().as_bytes();
    // `max_by_key` keeps the last of equal counts: comma.
    b"\t;,".iter().copied().filter(|d| first.contains(d)).max_by_key(|d| first.iter().filter(|b| *b == d).count())
}

/// A JSON value as a CSV cell: strings as they are, `null` as an empty cell,
/// arrays and objects as JSON text.
fn cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Adds the cells of `value` to `row`, nested objects as `key.child` columns.
fn flatten(key: &str, value: &Value, row: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (child, v) in map { flatten(&format!("{key}.{child}"), v, row); }
        }
        other => row.push((key.to_owned(), cell(other))),
    }
}

/// CSV from an array of objects (one column per key, in order of appearance),
/// a single object, an array of arrays (rows without a header) or an array of
/// plain values (one "value" column).
fn json_to_csv(input: &str, delimiter: u8) -> Result<String, String> {
    let value: Value = serde_json::from_str(input).map_err(|e| format!("Invalid JSON: {e}"))?;
    let items = match value {
        Value::Array(items) => items,
        object @ Value::Object(_) => vec![object],
        _ => return Err("Expected an array of objects, or one object".into()),
    };
    let mut writer = csv::WriterBuilder::new().delimiter(delimiter).terminator(csv::Terminator::Any(b'\n')).flexible(true).from_writer(Vec::new());
    let mut write = |record: Vec<String>| writer.write_record(record).map_err(|e| e.to_string());
    if items.iter().all(Value::is_object) {
        let rows: Vec<Vec<(String, String)>> = items.iter().filter_map(Value::as_object).map(|object| {
            let mut row = Vec::new();
            for (key, value) in object { flatten(key, value, &mut row); }
            row
        }).collect();
        let mut seen = HashSet::new();
        let columns: Vec<&str> = rows.iter().flatten().map(|(key, _)| key.as_str()).filter(|key| seen.insert(*key)).collect();
        write(columns.iter().map(|c| (*c).to_owned()).collect())?;
        for row in &rows {
            let cells: HashMap<&str, &str> = row.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            write(columns.iter().map(|c| cells.get(c).copied().unwrap_or_default().to_owned()).collect())?;
        }
    } else if items.iter().all(Value::is_array) {
        for row in items.iter().filter_map(Value::as_array) { write(row.iter().map(cell).collect())?; }
    } else if items.iter().all(|item| !item.is_object() && !item.is_array()) {
        write(vec!["value".into()])?;
        for item in &items { write(vec![cell(item)])?; }
    } else {
        return Err("The array mixes objects, arrays and plain values: use one kind".into());
    }
    let bytes = writer.into_inner().map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    Ok(text.trim_end_matches('\n').to_owned())
}

/// A CSV cell as JSON. With `detect`, an empty cell is `null`, and `true`,
/// `false`, numbers written the way JSON writes them (`42`, `-1.5`, not `007`)
/// and JSON arrays or objects keep their type; anything else is a string.
fn typed(text: &str, detect: bool) -> Value {
    if !detect { return Value::String(text.to_owned()); }
    let t = text.trim();
    match t {
        "" => Value::Null,
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ if t.starts_with(['[', '{']) => serde_json::from_str(t).unwrap_or_else(|_| Value::String(text.to_owned())),
        _ => match serde_json::from_str::<serde_json::Number>(t) {
            // Only when nothing is lost: "007" or "1.50" stay strings.
            Ok(number) if number.to_string() == t => Value::Number(number),
            _ => Value::String(text.to_owned()),
        },
    }
}

/// Puts `value` at `key`, where `a.b` goes into the object `a`.
fn insert(map: &mut Map<String, Value>, key: &str, value: Value) {
    if let Some((parent, child)) = key.split_once('.') {
        if let Value::Object(inner) = map.entry(parent).or_insert_with(|| Value::Object(Map::new())) {
            insert(inner, child, value);
            return;
        }
    }
    map.insert(key.to_owned(), value);
}

/// JSON from CSV: an array of objects keyed by the header row, or with
/// `header` off, an array of arrays. Columns named `a.b` become nested
/// objects unless a column is also named `a` (or a name has an empty part).
/// Cells beyond the header are named `column_N`.
fn csv_to_json(input: &str, delimiter: u8, header: bool, detect: bool) -> Result<String, String> {
    let mut reader = csv::ReaderBuilder::new().delimiter(delimiter).has_headers(false).flexible(true).from_reader(input.as_bytes());
    let records: Vec<csv::StringRecord> = reader.records().collect::<Result<_, _>>().map_err(|e| format!("Invalid CSV: {e}"))?;
    let rows: Vec<Value> = match (header, records.split_first()) {
        (true, Some((names, body))) => {
            let names: Vec<String> = names.iter().map(|name| name.trim().to_owned()).collect();
            let nest = names.iter().all(|name| !name.split('.').any(str::is_empty))
                && !names.iter().any(|name| names.iter().any(|other| other.starts_with(&format!("{name}."))));
            body.iter().map(|record| {
                let mut object = Map::new();
                for (i, text) in record.iter().enumerate() {
                    let key = names.get(i).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| format!("column_{}", i + 1));
                    let value = typed(text, detect);
                    if nest { insert(&mut object, &key, value); } else { object.insert(key, value); }
                }
                Value::Object(object)
            }).collect()
        }
        (true, None) => Vec::new(),
        (false, _) => records.iter().map(|record| Value::Array(record.iter().map(|text| typed(text, detect)).collect())).collect(),
    };
    serde_json::to_string_pretty(&Value::Array(rows)).map_err(|e| e.to_string())
}

/// The JSON ↔ CSV tool. The mode is `io.active`: 0 is JSON → CSV.
pub struct JsonCsv {
    io: TextIo,
    delimiter: Delimiter,
    /// CSV → JSON: the first row names the columns.
    header: bool,
    /// CSV → JSON: detect numbers, booleans, empty cells and JSON values.
    detect_types: bool,
}

impl Default for JsonCsv {
    fn default() -> Self {
        Self { io: TextIo { input_syntax: Some("json"), ..TextIo::default() }, delimiter: Delimiter::Auto, header: true, detect_types: true }
    }
}

impl JsonCsv {
    /// Converts the input in the selected direction.
    fn run(&mut self) {
        let to_csv = self.io.active == 0;
        self.io.input_syntax = to_csv.then_some("json");
        self.io.output_syntax = (!to_csv).then_some("json");
        if self.io.input.trim().is_empty() {
            self.io.output.clear();
            self.io.error.clear();
            return;
        }
        let result = if to_csv {
            json_to_csv(&self.io.input, self.delimiter.writing())
        } else {
            csv_to_json(&self.io.input, self.delimiter.reading(&self.io.input), self.header, self.detect_types)
        };
        self.io.apply(result);
    }
}

impl Tool for JsonCsv {
    fn name(&self) -> &'static str { "JSON \u{2194} CSV" }
    fn icon(&self) -> &'static str { icon::TABLE }
    fn category(&self) -> Category { Category::Data }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // The ⇄ menu leaves out the tool it is drawn in.
        let tool_name = self.name();
        ui::description(ui, "Converts an array of JSON objects to CSV and back. Nested objects become columns like address.city.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let (delimiter, header, detect) = (&mut self.delimiter, &mut self.header, &mut self.detect_types);
            let run_now = self.io.input_pane_with(left, &MODES, "Paste JSON or CSV here…", Some(SAMPLE), |ui| {
                let mut changed = false;
                ui.menu_button(format!("{}  Options", icon::SLIDERS_HORIZONTAL), |ui| {
                    ui.label(egui::RichText::new("Delimiter").small().color(theme::SUBTLE));
                    for choice in Delimiter::ALL { changed |= ui.radio_value(delimiter, choice, choice.label()).changed(); }
                    ui.separator();
                    ui.label(egui::RichText::new("CSV \u{2192} JSON").small().color(theme::SUBTLE));
                    changed |= ui.checkbox(header, "First row is the header").changed();
                    changed |= ui.checkbox(detect, "Detect numbers, booleans and empty cells").changed();
                });
                changed
            });
            if run_now { self.run(); }
            let reuse = self.io.output_pane(right, |ui, output| super::reuse_menu(ui, output, tool_name));
            if reuse {
                // The result is in the other format: convert it back.
                self.io.input = std::mem::take(&mut self.io.output);
                self.io.active = 1 - self.io.active;
                self.run();
            }
        });
    }

    fn load_sample(&mut self) {
        self.io.input = SAMPLE.into();
        self.io.active = 0;
        self.run();
    }

    fn accepts_input(&self) -> bool { true }

    /// JSON is converted to CSV, anything else is read as CSV.
    fn set_input(&mut self, text: &str) {
        self.io.input = text.to_owned();
        self.io.active = usize::from(!text.trim_start().starts_with(['[', '{']));
        self.run();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_to_csv_flattens_objects() {
        let csv = json_to_csv(SAMPLE, b',').unwrap();
        assert_eq!(csv, "id,name,active,address.city,address.country,tags\n\
            1,Ada Lovelace,true,London,UK,\"[\"\"math\"\",\"\"poetry\"\"]\"\n\
            2,Grace Hopper,false,New York,US,[]\n\
            3,\"Alan Turing, OBE\",true,Wilmslow,,");
        assert_eq!(json_to_csv("[[1, \"a\"], [2, null]]", b';').unwrap(), "1;a\n2;");
        assert_eq!(json_to_csv("[\"x\", 2]", b',').unwrap(), "value\nx\n2");
        assert!(json_to_csv("[{\"a\": 1}, 2]", b',').is_err());
        assert!(json_to_csv("42", b',').is_err());
        assert!(json_to_csv("{", b',').is_err());
    }

    #[test]
    fn csv_to_json_round_trips() {
        let csv = json_to_csv(SAMPLE, b',').unwrap();
        let back: Value = serde_json::from_str(&csv_to_json(&csv, b',', true, true).unwrap()).unwrap();
        let mut expected: Value = serde_json::from_str(SAMPLE).unwrap();
        // The third person had no tags: the empty cell comes back as null.
        expected[2]["tags"] = Value::Null;
        assert_eq!(back, expected);
    }

    #[test]
    fn csv_options() {
        assert_eq!(Delimiter::Auto.reading("a;b;c\n1;2;3"), b';');
        assert_eq!(Delimiter::Auto.reading("a\tb,c\td"), b'\t');
        assert_eq!(Delimiter::Auto.reading("single"), b',');
        let json = csv_to_json("zip,price\n007,1.50\n42,-3", b',', true, true).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), serde_json::json!([{"zip": "007", "price": "1.50"}, {"zip": 42, "price": -3}]));
        let raw = csv_to_json("a,b\n1,true\n2", b',', false, false).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(), serde_json::json!([["a", "b"], ["1", "true"], ["2"]]));
        // A column named "a" keeps "a.b" flat; an extra cell gets a name.
        let flat = csv_to_json("a,a.b\n1,2,3", b',', true, true).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&flat).unwrap(), serde_json::json!([{"a": 1, "a.b": 2, "column_3": 3}]));
        assert_eq!(csv_to_json("", b',', true, true).unwrap(), "[]");
    }
}
