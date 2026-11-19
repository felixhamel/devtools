//! JSON ↔ YAML ↔ TOML: reads any of the three (the format is detected) and
//! writes the one chosen. YAML anchors and merge keys (`<<: *defaults`) are
//! expanded, and a YAML stream of several documents becomes an array.
use eframe::egui;
use serde_json::Value;
use super::{Category, Tool};
use crate::ui::{self, icon, theme, TextIo};

/// Example for the "Sample" button: a YAML service with an anchor merged twice.
const SAMPLE: &str = "# Orders API
service:
  name: orders-api
  version: 1.4.2
  replicas: 3
  public: true
  tags: [api, orders]
defaults: &defaults
  timeout: 30
  retries: 2
endpoints:
  - path: /orders
    method: GET
    <<: *defaults
  - path: /orders/{id}
    method: DELETE
    timeout: 5
    <<: *defaults
";

/// A data format the tool reads and writes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Format { Json, Yaml, Toml }

impl Format {
    /// Every format, in the order of the output switch.
    const ALL: [Format; 3] = [Format::Json, Format::Yaml, Format::Toml];

    fn label(self) -> &'static str {
        match self { Format::Json => "JSON", Format::Yaml => "YAML", Format::Toml => "TOML" }
    }

    /// Highlighting of text in this format (TOML has no syntax definition, so it stays plain).
    fn syntax(self) -> Option<&'static str> {
        match self { Format::Json => Some("json"), Format::Yaml => Some("yaml"), Format::Toml => None }
    }

    /// Position in [`Format::ALL`], which is also the index of the output switch.
    fn index(self) -> usize { self as usize }
}

/// The documents of a YAML stream, with YAML 1.2's booleans: only `true` and
/// `false` (in any case) are booleans, so `yes`, `on` or the country code `NO`
/// stay text instead of silently becoming `true` or `false`.
pub(super) fn yaml_documents(text: &str) -> Result<Vec<Value>, serde_saphyr::Error> {
    serde_saphyr::from_multiple_with_options(text, serde_saphyr::options! { strict_booleans: true })
}

/// Reads `text` as JSON if it is valid JSON, as TOML if it is valid TOML, and
/// as YAML otherwise. A YAML stream of several documents becomes an array.
fn read(text: &str) -> Result<(Format, Value), String> {
    let json_error = match serde_json::from_str::<Value>(text) {
        Ok(value) => return Ok((Format::Json, value)),
        Err(e) => e,
    };
    if let Ok(table) = toml::from_str::<toml::Table>(text) {
        return Ok((Format::Toml, from_toml(toml::Value::Table(table))));
    }
    // Text that starts like JSON (and is not a TOML `[table]`) is meant as JSON:
    // YAML would accept some broken JSON, such as `{"a": }`, and hide the mistake.
    if text.trim_start().starts_with(['{', '[']) { return Err(format!("Invalid JSON: {json_error}")); }
    match yaml_documents(text) {
        Ok(mut documents) if documents.len() == 1 => Ok((Format::Yaml, documents.remove(0))),
        Ok(documents) => Ok((Format::Yaml, Value::Array(documents))),
        // The first line says where and what ("line 2 column 4: …"); the snippet
        // under it needs a monospace font to point at the right column.
        Err(e) => Err(format!("Invalid YAML: {}", e.to_string().lines().next().unwrap_or_default().trim_start_matches("error: "))),
    }
}

/// A TOML value as JSON. Dates become strings, and floats JSON cannot hold
/// (`nan`, `inf`) become strings too.
fn from_toml(value: toml::Value) -> Value {
    match value {
        toml::Value::String(text) => Value::String(text),
        toml::Value::Integer(n) => n.into(),
        toml::Value::Float(f) => serde_json::Number::from_f64(f).map_or_else(|| Value::String(f.to_string()), Value::Number),
        toml::Value::Boolean(b) => b.into(),
        toml::Value::Datetime(date) => date.to_string().into(),
        toml::Value::Array(items) => items.into_iter().map(from_toml).collect(),
        toml::Value::Table(table) => Value::Object(table.into_iter().map(|(k, v)| (k, from_toml(v))).collect()),
    }
}

/// A JSON value as TOML. `path` names the value in errors, since TOML has no
/// null and integers stop at 64 bits.
fn to_toml(value: &Value, path: &str) -> Result<toml::Value, String> {
    Ok(match value {
        Value::Null => return Err(format!("TOML has no null, and {path} is null: remove it or give it a value")),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => toml::Value::Integer(i),
            (None, Some(f)) if !n.is_u64() => toml::Value::Float(f),
            _ => return Err(format!("{path} is too large for a TOML integer")),
        },
        Value::String(text) => toml::Value::String(text.clone()),
        Value::Array(items) => toml::Value::Array(items.iter().enumerate()
            .map(|(i, item)| to_toml(item, &format!("{path}[{i}]"))).collect::<Result<_, _>>()?),
        Value::Object(map) => toml::Value::Table(map.iter()
            .map(|(key, item)| Ok((key.clone(), to_toml(item, &if path.is_empty() { key.clone() } else { format!("{path}.{key}") })?)))
            .collect::<Result<_, String>>()?),
    })
}

/// `value` written in `format`.
fn write(value: &Value, format: Format) -> Result<String, String> {
    match format {
        Format::Json => serde_json::to_string_pretty(value).map_err(|e| e.to_string()),
        Format::Yaml => serde_saphyr::to_string(value).map_err(|e| e.to_string()),
        Format::Toml => match to_toml(value, "")? {
            toml::Value::Table(table) => toml::to_string_pretty(&table).map_err(|e| e.to_string()),
            _ => Err("TOML needs an object at the top level, not a list or a single value".into()),
        },
    }
}

/// Reads `input` and writes it in `target`; also returns the format it was read as.
fn convert(input: &str, target: Format) -> (Option<Format>, Result<String, String>) {
    match read(input) {
        Ok((source, value)) => (Some(source), write(&value, target).map(|text| text.trim_end().to_owned())),
        Err(e) => (None, Err(e)),
    }
}

/// The JSON ↔ YAML ↔ TOML tool. The output format is `io.active`, an index into [`Format::ALL`].
#[derive(Default)]
pub struct YamlToml {
    io: TextIo,
    /// The format the input was read as, once it could be read.
    source: Option<Format>,
}

impl YamlToml {
    /// Converts the input to the selected format.
    fn run(&mut self) {
        let target = Format::ALL[self.io.active];
        self.io.output_syntax = target.syntax();
        if self.io.input.trim().is_empty() {
            self.source = None;
            self.io.output.clear();
            self.io.error.clear();
            return;
        }
        let (source, result) = convert(&self.io.input, target);
        // Keep the input's highlighting while it has an error.
        if source.is_some() { self.source = source; }
        self.io.input_syntax = self.source.and_then(Format::syntax);
        self.io.apply(result);
    }
}

impl Tool for YamlToml {
    fn name(&self) -> &'static str { "JSON \u{2194} YAML \u{2194} TOML" }
    fn icon(&self) -> &'static str { icon::SWAP }
    fn category(&self) -> Category { Category::Data }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // The ⇄ menu leaves out the tool it is drawn in.
        let tool_name = self.name();
        ui::description(ui, "Paste JSON, YAML or TOML (the format is detected), then pick the format to convert to.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let targets: Vec<String> = Format::ALL.iter().map(|f| format!("To {}", f.label())).collect();
            let targets: Vec<&str> = targets.iter().map(String::as_str).collect();
            if self.io.input_pane(left, &targets, "Paste JSON, YAML or TOML here…", Some(SAMPLE)) { self.run(); }
            let source = self.source.filter(|_| self.io.error.is_empty());
            let reuse = self.io.output_pane(right, |ui, output| {
                let reuse = super::reuse_menu(ui, output, tool_name);
                if let Some(source) = source {
                    ui.label(egui::RichText::new(format!("Read as {}", source.label())).small().color(theme::SUBTLE));
                }
                reuse
            });
            if reuse {
                // Convert the result back to the format it came from.
                self.io.input = std::mem::take(&mut self.io.output);
                self.io.active = source.map_or(0, Format::index);
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

    /// JSON is converted to YAML, anything else to JSON.
    fn set_input(&mut self, text: &str) {
        self.io.input = text.to_owned();
        self.io.active = match read(text) { Ok((Format::Json, _)) => Format::Yaml.index(), _ => Format::Json.index() };
        self.run();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_yaml_with_merge_keys() {
        let (format, value) = read(SAMPLE).unwrap();
        assert_eq!(format, Format::Yaml);
        assert_eq!(value["service"]["tags"], json!(["api", "orders"]));
        assert_eq!(value["service"]["version"], json!("1.4.2"));
        // Merged keys fill in; explicit ones win.
        assert_eq!(value["endpoints"][0], json!({"path": "/orders", "method": "GET", "timeout": 30, "retries": 2}));
        assert_eq!(value["endpoints"][1]["timeout"], json!(5));
        // Several documents become an array.
        assert_eq!(read("a: 1\n---\nb: 2\n").unwrap(), (Format::Yaml, json!([{"a": 1}, {"b": 2}])));
    }

    #[test]
    fn detects_each_format() {
        assert_eq!(read("{\"a\": [1, 2]}").unwrap().0, Format::Json);
        assert_eq!(read("title = \"x\"\n[owner]\nname = \"Ada\"").unwrap(), (Format::Toml, json!({"title": "x", "owner": {"name": "Ada"}})));
        assert_eq!(read("when = 2026-11-19T10:00:00Z").unwrap().1, json!({"when": "2026-11-19T10:00:00Z"}));
        assert_eq!(read("- a\n- b").unwrap(), (Format::Yaml, json!(["a", "b"])));
        // Broken JSON reports the JSON error.
        assert!(read("{\"a\": }").unwrap_err().starts_with("Invalid JSON"));
        assert_eq!(read("a: 1\n  b: 2").unwrap_err(), "Invalid YAML: line 2 column 4: mapping values are not allowed in this context");
    }

    #[test]
    fn writes_each_format() {
        let value = json!({"name": "api", "port": 8080, "ratio": 0.5, "tags": ["a", "b"], "db": {"host": "localhost"}});
        assert_eq!(write(&value, Format::Json).unwrap(), serde_json::to_string_pretty(&value).unwrap());
        let yaml = write(&value, Format::Yaml).unwrap();
        assert_eq!(read(&yaml).unwrap(), (Format::Yaml, value.clone()));
        let toml = write(&value, Format::Toml).unwrap();
        assert!(toml.contains("name = \"api\"\nport = 8080\n") && toml.contains("[db]\nhost = \"localhost\""), "{toml}");
        assert_eq!(read(&toml).unwrap(), (Format::Toml, value));
    }

    #[test]
    fn keeps_yaml_text_as_text() {
        // YAML 1.2: only true and false are booleans ("NO" is Norway), and `on:` stays a key.
        let (_, value) = read("on: push\ncountry: NO\nenabled: yes\nreal: True\nversion: \"1.10\"\nempty:\ntilde: ~").unwrap();
        assert_eq!(value, json!({"on": "push", "country": "NO", "enabled": "yes", "real": true, "version": "1.10", "empty": null, "tilde": null}));
        // An alias repeats the anchored value.
        assert_eq!(read("a: &x [1, 2]\nb: *x").unwrap().1, json!({"a": [1, 2], "b": [1, 2]}));
        // Strings that look like other types keep their quotes when written.
        let quoted = json!({"s": "123", "t": "true", "n": "null", "e": "", "multi": "a\nb", "colon": "a: b"});
        assert_eq!(read(&write(&quoted, Format::Yaml).unwrap()).unwrap().1, quoted);
    }

    #[test]
    fn converts_between_toml_and_yaml() {
        let toml = "title = \"Orders\"\n\n[database]\nports = [8000, 8001]\nenabled = true\n\n[[owners]]\nname = \"Ada\"\n\n[[owners]]\nname = \"Grace\"";
        let (format, value) = read(toml).unwrap();
        assert_eq!(format, Format::Toml);
        assert_eq!(value["owners"][1], json!({"name": "Grace"}));
        let yaml = write(&value, Format::Yaml).unwrap();
        assert_eq!(read(&yaml).unwrap(), (Format::Yaml, value.clone()));
        // Arrays of objects become [[tables]] again.
        let back = write(&value, Format::Toml).unwrap();
        assert!(back.contains("[[owners]]\nname = \"Ada\"\n\n[[owners]]\nname = \"Grace\""), "{back}");
        // Floats JSON cannot hold become strings.
        assert_eq!(read("x = nan\ny = inf").unwrap().1, json!({"x": "NaN", "y": "inf"}));
    }

    #[test]
    fn picks_the_output_format_from_the_input() {
        let mut tool = YamlToml::default();
        tool.set_input("{\"name\": \"api\"}");
        assert_eq!((tool.io.active, tool.io.output.as_str(), tool.source), (Format::Yaml.index(), "name: api", Some(Format::Json)));
        tool.set_input("name: api");
        assert_eq!((tool.io.active, tool.io.output.as_str(), tool.source), (Format::Json.index(), "{\n  \"name\": \"api\"\n}", Some(Format::Yaml)));
        // An error clears the output but keeps the last format read (for the input's highlighting).
        tool.set_input("{\"name\": }");
        assert!(tool.io.error.starts_with("Invalid JSON") && tool.io.output.is_empty(), "{}", tool.io.error);
        assert_eq!(tool.source, Some(Format::Yaml));
        tool.set_input("  ");
        assert!(tool.io.error.is_empty() && tool.io.output.is_empty() && tool.source.is_none());
    }

    #[test]
    fn explains_what_toml_cannot_hold() {
        assert_eq!(write(&json!({"a": {"b": [1, null]}}), Format::Toml).unwrap_err(),
            "TOML has no null, and a.b[1] is null: remove it or give it a value");
        assert!(write(&json!([1, 2]), Format::Toml).unwrap_err().contains("object at the top level"));
        assert!(write(&json!({"big": u64::MAX}), Format::Toml).unwrap_err().contains("too large"));
    }
}
