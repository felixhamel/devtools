//! CloudWatch Logs Decoder: turns the gzip-compressed Base64 that CloudWatch
//! Logs subscriptions deliver (to Lambda, Kinesis or Firehose) back into
//! readable log events and JSON. Any gzip + Base64 text works too.
use std::io::Read;
use base64::{engine::general_purpose::{STANDARD, URL_SAFE}, Engine};
use eframe::egui;
use serde_json::Value;
use super::{Category, Tool};
use crate::ui::{self, icon, TextIo};

/// Example for the "Sample" button: the event a Lambda function receives from
/// a CloudWatch Logs subscription, with three log events.
const SAMPLE: &str = r#"{
  "awslogs": {
    "data": "H4sIAAAAAAAC/62RT2vCQBDFv0oYekxIdhOjyS1gKkJbigm9GJFNdpTQ/OtuVET87h1jC148SHsbZh5vf+/tCWrUWmwxPXYIIUyjNFq/xkkSzWIwoT00qGjNuOuN/PEkcBinddVuZ6rddXSxxUHblahzKexWSVTaEl151SS9QlGTiDvctxmzWWAvn16iNE7Slb9hBZcuemKUk1zvcl2osuvLtnkuq56MIFwCKtWSZd9auhLFJ6wG43iPTX+5n6CU5O9ObvgemOjdvqQCelFTFjYORk7gew6jo/lbDNknabRIjQV+7Ug6l6Ex2XDhFwwtLx9Ly8NAWEHh5haTHN3NJZFfGB8UgcKExk/irIGz+Tdg/x4waW6BTxlUuMcqgzC7VpiBmUGtt8PmXRxrKtCQWFRlg3I4Dp83l4OA5jVzPJ7B+R+ox/eoSXBLHb9NHy35Qrc6fwMWDQmBxQIAAA=="
  }
}"#;

/// Decompressed data larger than this is refused, so a small paste cannot
/// fill the memory (a "zip bomb").
const MAX_DECOMPRESSED: u64 = 64 * 1024 * 1024;

/// What the output shows.
#[derive(Clone, Copy, PartialEq, Debug)]
enum View {
    /// One line per log event, after a summary of the log group and stream.
    Events,
    /// The decoded payloads as JSON (or text, when they are not JSON).
    Json,
}

/// The Base64 data to decode in `input`: every `data` string of a JSON event
/// (Lambda's `awslogs.data`, Kinesis' `Records[].kinesis.data`, Firehose's
/// `records[].data`), or the input itself when it is not JSON.
fn encoded_parts(input: &str) -> Result<Vec<String>, String> {
    /// Collects the strings found under a `data` key, at any depth.
    fn collect(value: &Value, found: &mut Vec<String>) {
        match value {
            Value::Object(map) => for (key, item) in map {
                match item {
                    Value::String(text) if key == "data" => found.push(text.clone()),
                    _ => collect(item, found),
                }
            },
            Value::Array(items) => for item in items { collect(item, found); },
            _ => {}
        }
    }
    let t = input.trim();
    match serde_json::from_str::<Value>(t) {
        Ok(Value::String(text)) => Ok(vec![text]),
        Ok(value) => {
            let mut found = Vec::new();
            collect(&value, &mut found);
            if found.is_empty() { Err("No \"data\" field found: paste the event, or the Base64 data itself".into()) } else { Ok(found) }
        }
        // Not JSON: the Base64 itself, perhaps quoted.
        Err(_) => Ok(vec![t.trim_matches(['"', '\'']).to_owned()]),
    }
}

/// Decodes Base64 (standard or URL-safe, line breaks allowed) and decompresses the gzip inside.
fn gunzip_base64(data: &str) -> Result<String, String> {
    let compact: String = data.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = STANDARD.decode(&compact).or_else(|_| URL_SAFE.decode(&compact)).map_err(|_| "Not valid Base64".to_owned())?;
    gunzip(&bytes, MAX_DECOMPRESSED)
}

/// The UTF-8 text compressed in `bytes` (gzip, possibly several members), if
/// it is at most `limit` bytes once decompressed.
fn gunzip(bytes: &[u8], limit: u64) -> Result<String, String> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Err("Not gzip-compressed data (CloudWatch Logs data starts with H4sI): try the Base64 String tool".into());
    }
    let mut text = String::new();
    flate2::read::MultiGzDecoder::new(bytes).take(limit + 1).read_to_string(&mut text)
        .map_err(|e| format!("Cannot decompress: {e}"))?;
    if text.len() as u64 > limit { return Err(format!("Larger than {} MB once decompressed", limit / (1024 * 1024))); }
    Ok(text)
}

/// A log event's time (milliseconds since 1970) in UTC, to the millisecond.
fn event_time(millis: i64) -> String {
    chrono::DateTime::from_timestamp_millis(millis).map_or_else(|| millis.to_string(), |t| t.format("%Y-%m-%d %H:%M:%S%.3fZ").to_string())
}

/// A CloudWatch Logs message as text: its log group, stream, account and
/// filters, then one line per event (time, then the message).
fn events_text(message: &Value) -> Option<String> {
    let events = message.get("logEvents")?.as_array()?;
    let field = |key: &str| message.get(key).and_then(Value::as_str).unwrap_or_default();
    let filters: Vec<&str> = message.get("subscriptionFilters").and_then(Value::as_array).map(|list| list.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let mut lines = vec![
        format!("Log group: {}", field("logGroup")),
        format!("Log stream: {}", field("logStream")),
        format!("Account: {}", field("owner")),
    ];
    if !filters.is_empty() { lines.push(format!("Subscription filters: {}", filters.join(", "))); }
    lines.push(format!("{} event{}", events.len(), if events.len() == 1 { "" } else { "s" }));
    lines.push(String::new());
    for event in events {
        let time = event.get("timestamp").and_then(Value::as_i64).map(event_time).unwrap_or_default();
        let text = event.get("message").and_then(Value::as_str).unwrap_or_default();
        // Lambda messages end with a line break; continuation lines are indented under the message.
        lines.push(format!("{time}  {}", text.trim_end().replace('\n', "\n                          ")));
    }
    Some(lines.join("\n"))
}

/// Both views of the decoded `input`: (events, JSON). The events view shows
/// payloads without `logEvents` (data that is not from CloudWatch Logs) as in
/// the JSON view.
fn decode(input: &str) -> Result<(String, String), String> {
    let mut events = Vec::new();
    let mut payloads = Vec::new();
    for part in encoded_parts(input)? {
        let text = gunzip_base64(&part)?;
        match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                let pretty = serde_json::to_string_pretty(&value).unwrap_or(text);
                events.push(events_text(&value).unwrap_or_else(|| pretty.clone()));
                payloads.push(pretty);
            }
            Err(_) => { events.push(text.clone()); payloads.push(text); }
        }
    }
    Ok((events.join("\n\n"), payloads.join("\n\n")))
}

/// The CloudWatch Logs Decoder tool.
pub struct CloudWatch {
    io: TextIo,
    view: View,
    /// The two views of the last result; `io.output` holds the one shown.
    events: String,
    json: String,
}

impl Default for CloudWatch {
    fn default() -> Self { Self { io: TextIo::default(), view: View::Events, events: String::new(), json: String::new() } }
}

impl CloudWatch {
    /// Decodes the input and shows the selected view.
    fn run(&mut self) {
        self.io.input_syntax = self.io.input.trim_start().starts_with('{').then_some("json");
        let result = if self.io.input.trim().is_empty() { Ok((String::new(), String::new())) } else { decode(&self.io.input) };
        match result {
            Ok((events, json)) => { self.events = events; self.json = json; self.io.error.clear(); }
            Err(e) => { self.events.clear(); self.json.clear(); self.io.error = e; }
        }
        self.show_view();
    }

    /// Puts the selected view in the output.
    fn show_view(&mut self) {
        let (text, syntax) = match self.view { View::Events => (&self.events, None), View::Json => (&self.json, Some("json")) };
        self.io.output.clone_from(text);
        self.io.output_syntax = syntax;
    }
}

impl Tool for CloudWatch {
    fn name(&self) -> &'static str { "CloudWatch Logs Decoder" }
    fn icon(&self) -> &'static str { icon::CLOUD_ARROW_DOWN }
    fn category(&self) -> Category { Category::Encoding }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // The ⇄ menu leaves out the tool it is drawn in.
        let tool_name = self.name();
        ui::description(ui, "Decodes CloudWatch Logs subscription data (gzip + Base64): paste the Lambda, Kinesis or Firehose event, or the data alone.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            if self.io.input_pane(left, &[], "Paste an event or its Base64 data (H4sI…)…", Some(SAMPLE)) { self.run(); }
            let view = &mut self.view;
            let (reuse, view_changed) = self.io.output_pane(right, |ui, output| {
                let reuse = super::reuse_menu(ui, output, tool_name);
                ui.separator();
                let (events, json) = (format!("{}  Log events", icon::LIST_BULLETS), format!("{}  JSON", icon::BRACKETS_CURLY));
                let changed = ui::segmented(ui, view, &[(View::Events, events.as_str()), (View::Json, json.as_str())]);
                (reuse, changed)
            });
            if view_changed { self.show_view(); }
            if reuse { self.io.input = std::mem::take(&mut self.io.output); self.run(); }
        });
    }

    fn load_sample(&mut self) {
        self.io.input = SAMPLE.into();
        self.run();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.io.input = text.to_owned();
        self.run();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// `bytes` gzip-compressed.
    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    /// `text` gzip-compressed and Base64-encoded, as CloudWatch Logs sends it.
    fn pack(text: &str) -> String { STANDARD.encode(gzip(text.as_bytes())) }

    #[test]
    fn decodes_a_lambda_event() {
        let (events, json) = decode(SAMPLE).unwrap();
        assert!(events.starts_with("Log group: /aws/lambda/orders-api\nLog stream: 2026/11/19/[$LATEST]6f1c2d3e4a5b\nAccount: 123456789012\nSubscription filters: errors-to-slack\n3 events\n\n"), "{events}");
        assert!(events.contains("\n2026-11-19 13:53:21.456Z  {\"level\":\"error\",\"msg\":\"Payment declined\",\"orderId\":\"ord_1042\"}\n"), "{events}");
        assert!(json.contains("\"messageType\": \"DATA_MESSAGE\""), "{json}");
    }

    #[test]
    fn decodes_kinesis_records_and_plain_data() {
        let message = r#"{"messageType":"DATA_MESSAGE","logGroup":"g","logStream":"s","owner":"1","logEvents":[{"timestamp":0,"message":"line one\nline two"}]}"#;
        let kinesis = format!(r#"{{"Records": [{{"kinesis": {{"data": "{0}"}}}}, {{"kinesis": {{"data": "{0}"}}}}]}}"#, pack(message));
        let (events, _) = decode(&kinesis).unwrap();
        assert_eq!(events.matches("Log group: g").count(), 2, "{events}");
        assert!(events.contains("1970-01-01 00:00:00.000Z  line one\n                          line two"), "{events}");
        // The data alone, wrapped over several lines; text that is not JSON stays as it is.
        let data = pack("plain text");
        let (events, json) = decode(&format!("{}\n{}", &data[..10], &data[10..])).unwrap();
        assert_eq!((events.as_str(), json.as_str()), ("plain text", "plain text"));
    }

    #[test]
    fn decodes_every_event_shape() {
        let message = r#"{"messageType":"DATA_MESSAGE","logGroup":"g","logStream":"s","owner":"1","logEvents":[]}"#;
        // A Firehose transformation event.
        let firehose = format!(r#"{{"invocationId": "x", "records": [{{"recordId": "1", "data": "{}"}}]}}"#, pack(message));
        assert!(decode(&firehose).unwrap().0.ends_with("Account: 1\n0 events\n"), "{:?}", decode(&firehose));
        // The data as a JSON string, or in the URL-safe alphabet.
        assert_eq!(decode(&format!("\"{}\"", pack("hi"))).unwrap(), ("hi".to_owned(), "hi".to_owned()));
        assert_eq!(decode(&URL_SAFE.encode(gzip(b"url-safe"))).unwrap().0, "url-safe");
        // The message CloudWatch sends to check a new subscription has events too.
        let control = r#"{"messageType":"CONTROL_MESSAGE","owner":"CloudwatchLogs","logGroup":"","logStream":"","subscriptionFilters":[],"logEvents":[{"id":"","timestamp":1795096401000,"message":"CWL CONTROL MESSAGE: Checking health of destination"}]}"#;
        assert!(decode(&pack(control)).unwrap().0.ends_with("2026-11-19 13:53:21.000Z  CWL CONTROL MESSAGE: Checking health of destination"));
    }

    #[test]
    fn decompresses_safely() {
        // Several gzip members in a row are read as one text.
        let two = [gzip(b"ab"), gzip(b"cd")].concat();
        assert_eq!(gunzip(&two, 100).unwrap(), "abcd");
        // Up to the limit, and not one byte more.
        assert_eq!(gunzip(&gzip(b"abc"), 3).unwrap(), "abc");
        assert!(gunzip(&gzip(&[b'a'; 1000]), 100).unwrap_err().starts_with("Larger than"));
        assert!(gunzip(&gzip(&[0xff, 0xfe]), 100).unwrap_err().starts_with("Cannot decompress"));
    }

    #[test]
    fn explains_bad_input() {
        assert!(decode("not base64!").unwrap_err().contains("Base64"));
        assert!(decode(&STANDARD.encode("hello")).unwrap_err().contains("Not gzip"));
        assert!(decode("{\"other\": 1}").unwrap_err().contains("\"data\""));
    }
}
