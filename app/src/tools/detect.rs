//! Recognizes what a pasted text is (a JWT, JSON, a timestamp, a URL…) to open
//! it in the right tool: ⌘V on the home screen and the palette's "Paste" command.
use regex::Regex;
use std::sync::LazyLock;

/// A recognized kind of text: the tool that handles it, and a short name for it.
#[derive(Debug, PartialEq, Eq)]
pub struct Detected {
    /// The [`Tool::name`](super::Tool::name) of the tool that opens it.
    pub tool: &'static str,
    /// What it is, for messages: "a JWT", "JSON", "an IP address"…
    pub what: &'static str,
}

/// Compiles one of the patterns below, which are constants known to be valid.
fn re(pattern: &str) -> Regex { Regex::new(pattern).expect("detection patterns are valid") }

/// Three dot-separated parts in URL-safe Base64; the header is JSON, so it starts with `eyJ` (`{"`).
static JWT: LazyLock<Regex> = LazyLock::new(|| re(r"^eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*$"));
/// An Amazon Resource Name: `arn:partition:service:`…
static ARN: LazyLock<Regex> = LazyLock::new(|| re(r"^arn:aws[a-z-]*:[a-z0-9-]+:"));
/// The first line of a YAML document: `---`, a list item, or a `key:`.
static YAML_START: LazyLock<Regex> = LazyLock::new(|| re(r"^(---\s*$|- |[A-Za-z_][\w.-]*:(\s|$))"));
/// With or without hyphens, optionally in braces or as a `urn:uuid:` URN.
static UUID: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(urn:uuid:)?\{?[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}\}?$"));
/// 26 Crockford Base32 characters; the first is at most 7 so the timestamp fits in 48 bits.
static ULID: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^[0-7][0-9A-HJKMNP-TV-Z]{25}$"));
/// 9 to 19 digits (seconds since 1973 up to nanoseconds), with an optional fraction.
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| re(r"^\d{9,19}(\.\d+)?$"));
/// An ISO 8601 date, optionally followed by a time.
static DATE: LazyLock<Regex> = LazyLock::new(|| re(r"^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}.*)?$"));
/// A hex color (3, 4, 6 or 8 digits) or a CSS color function such as `rgb(…)` or `oklch(…)`.
static COLOR: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})|(rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(.+\))$"));
/// An AWS EventBridge schedule: `cron(…)`, `rate(…)` or `at(…)`.
static EVENTBRIDGE: LazyLock<Regex> = LazyLock::new(|| re(r"^(cron|rate|at)\([^()]+\)$"));
/// One cron field: numbers, `* / , - ? L W #` and month or day names.
static CRON_FIELD: LazyLock<Regex> = LazyLock::new(|| re(r"^[0-9*/,?LW#A-Za-z-]+$"));
/// A percent-encoded byte, e.g. `%20`.
static PERCENT: LazyLock<Regex> = LazyLock::new(|| re(r"%[0-9A-Fa-f]{2}"));
/// A named or numeric HTML entity, e.g. `&amp;`, `&#169;` or `&#xA9;`.
static ENTITY: LazyLock<Regex> = LazyLock::new(|| re(r"&(#\d+|#x[0-9A-Fa-f]+|[A-Za-z]+);"));
/// At least four hex bytes, optionally separated by spaces or colons.
static HEX: LazyLock<Regex> = LazyLock::new(|| re(r"^([0-9A-Fa-f]{2}[\s:]?){4,}$"));
/// At least 8 characters of standard or URL-safe Base64, with optional padding.
static BASE64: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z0-9+/_-]{8,}={0,2}$"));
/// An identifier of two words or more: `snake_case`, `kebab-case`, `dot.case`, `camelCase` or `PascalCase`.
static IDENTIFIER: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z][A-Za-z0-9]*([_.-][A-Za-z0-9]+)+$|^[a-z]+([A-Z][a-z0-9]*)+$|^([A-Z][a-z0-9]+){2,}$"));

/// The tool that converts between JSON, YAML and TOML.
const DATA_FORMATS: &str = "JSON \u{2194} YAML \u{2194} TOML";

/// Words that start an SQL statement (compared in uppercase).
const SQL_KEYWORDS: [&str; 8] = ["SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "DROP", "WITH"];

/// The tool that best handles `text`, or `None` when nothing specific is recognized.
pub fn detect(text: &str) -> Option<Detected> {
    let t = text.trim();
    if t.is_empty() { return None; }
    let found = |tool, what| Some(Detected { tool, what });
    let one_word = !t.contains(char::is_whitespace);
    // From the most to the least specific format: the order matters, since a text
    // can match several patterns (e.g. a UUID without hyphens is also hex).
    if JWT.is_match(t) { return found("JWT Debugger", "a JWT"); }
    if ARN.is_match(t) { return found("AWS ARN Parser", "an ARN"); }
    if t.contains("-----BEGIN CERTIFICATE-----") { return found("Certificate Decoder (X.509)", "a certificate"); }
    if t.starts_with("data:image/") { return found("Base64 Image Encode/Decode", "an image data URI"); }
    if t.starts_with('{') && t.contains("\"awslogs\"") { return found("CloudWatch Logs Decoder", "a CloudWatch Logs event"); }
    if (t.starts_with('{') || t.starts_with('[')) && serde_json::from_str::<serde_json::Value>(t).is_ok() { return found("JSON Format/Validate", "JSON"); }
    // "H4sI" is the gzip header in Base64: CloudWatch Logs subscription data.
    if t.starts_with("H4sI") && BASE64.is_match(t) { return found("CloudWatch Logs Decoder", "gzip-compressed Base64"); }
    if one_word && t.contains("://") && url::Url::parse(t).is_ok_and(|u| u.has_host()) { return found("URL Parser", "a URL"); }
    if UUID.is_match(t) { return found("UUID/ULID Generate/Decode", "a UUID"); }
    if one_word && (t.parse::<ipnet::IpNet>().is_ok() || t.parse::<std::net::IpAddr>().is_ok()) {
        return found("IP Subnet Calculator", if t.contains('/') { "a CIDR block" } else { "an IP address" });
    }
    if ULID.is_match(t) { return found("UUID/ULID Generate/Decode", "a ULID"); }
    if TIMESTAMP.is_match(t) { return found("Unix Time Converter", "a timestamp"); }
    if DATE.is_match(t) { return found("Unix Time Converter", "a date"); }
    if COLOR.is_match(t) { return found("Color Converter", "a color"); }
    if EVENTBRIDGE.is_match(t) { return found("Cron Job Parser", "an EventBridge schedule"); }
    // 5 to 7 fields with at least one `*` or `/` (so that a five-word sentence
    // is not taken), and croner must accept it.
    let fields: Vec<&str> = t.split_whitespace().collect();
    if (5..=7).contains(&fields.len()) && fields.iter().all(|f| CRON_FIELD.is_match(f)) && fields.iter().any(|f| f.contains(['*', '/']))
        && t.parse::<croner::Cron>().is_ok() {
        return found("Cron Job Parser", "a cron expression");
    }
    if t.starts_with("<?xml") { return found("XML Beautify/Minify", "XML"); }
    if t.starts_with('<') && t.ends_with('>') { return found("HTML Beautify/Minify", "HTML"); }
    let first = fields.first().map(|w| w.to_ascii_uppercase()).unwrap_or_default();
    if SQL_KEYWORDS.contains(&first.as_str()) && fields.len() > 1 { return found("SQL Formatter", "SQL"); }
    if t.contains('=') && toml::from_str::<toml::Table>(t).is_ok_and(|table| !table.is_empty()) { return found(DATA_FORMATS, "TOML"); }
    if is_yaml(t) { return found(DATA_FORMATS, "YAML"); }
    if is_csv(t) { return found("JSON \u{2194} CSV", "CSV"); }
    if one_word && PERCENT.is_match(t) { return found("URL Encode/Decode", "URL-encoded text"); }
    if ENTITY.is_match(t) { return found("HTML Entity Encode/Decode", "HTML entities"); }
    // Hex needs both a digit and a letter, so plain numbers and words are left alone.
    if HEX.is_match(t) && t.chars().any(|c| c.is_ascii_digit()) && t.chars().any(|c| c.is_ascii_alphabetic()) { return found("Hex \u{2194} ASCII", "hex bytes"); }
    if BASE64.is_match(t) && t.len().is_multiple_of(4) && decodes_to_text(t) { return found("Base64 String Encode/Decode", "Base64"); }
    if IDENTIFIER.is_match(t) { return found("String Case Converter", "an identifier"); }
    None
}

/// Whether `text` is a YAML mapping or list of at least two lines (one line
/// such as "Note: hi" is left alone).
fn is_yaml(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#')).collect();
    lines.len() >= 2 && YAML_START.is_match(lines[0])
        && super::yaml_toml::yaml_documents(text).is_ok_and(|docs| docs.first().is_some_and(|d| d.is_object() || d.is_array()))
}

/// Whether `text` looks like a table: at least 3 lines that all have the same
/// number of fields (2 or more), split by the comma, semicolon or tab that the
/// first line uses most.
fn is_csv(text: &str) -> bool {
    if text.lines().filter(|l| !l.trim().is_empty()).count() < 3 { return false; }
    let Some(delimiter) = super::json_csv::guess_delimiter(text) else { return false };
    let mut reader = csv::ReaderBuilder::new().delimiter(delimiter).has_headers(false).from_reader(text.as_bytes());
    // Without `flexible`, the reader fails on a record of another length.
    let mut fields = 0;
    reader.records().all(|record| record.is_ok_and(|r| { fields = r.len(); r.len() >= 2 })) && fields >= 2
}

/// Whether `text` is Base64 for readable UTF-8 (so plain words are not mistaken for it).
fn decodes_to_text(text: &str) -> bool {
    use base64::{engine::general_purpose::{STANDARD, URL_SAFE}, Engine};
    let bytes = STANDARD.decode(text).or_else(|_| URL_SAFE.decode(text));
    bytes.ok().and_then(|b| String::from_utf8(b).ok()).is_some_and(|s| !s.is_empty() && s.chars().all(|c| !c.is_control() || c.is_whitespace()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(text: &str) -> Option<&'static str> { detect(text).map(|d| d.tool) }

    #[test]
    fn recognizes_common_formats() {
        let cases = [
            ("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl", "JWT Debugger"),
            ("-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----", "Certificate Decoder (X.509)"),
            ("data:image/png;base64,iVBORw0KGgo=", "Base64 Image Encode/Decode"),
            (" {\"name\": \"DevUtils\"} ", "JSON Format/Validate"),
            ("https://example.com/search?q=rust", "URL Parser"),
            ("3f2504e0-4f89-11d3-9a0c-0305e82c3301", "UUID/ULID Generate/Decode"),
            ("01ARZ3NDEKTSV4RRFFQ69G5FAV", "UUID/ULID Generate/Decode"),
            ("10.0.0.0/16", "IP Subnet Calculator"),
            ("192.168.1.10", "IP Subnet Calculator"),
            ("2600:1f18:abcd::/56", "IP Subnet Calculator"),
            ("1700000000", "Unix Time Converter"),
            ("1700000000123", "Unix Time Converter"),
            ("2026-11-18T10:30:00Z", "Unix Time Converter"),
            ("#82dca9", "Color Converter"),
            ("rgb(130, 220, 169)", "Color Converter"),
            ("*/15 9-17 * * MON-FRI", "Cron Job Parser"),
            ("cron(0 12 ? * MON-FRI *)", "Cron Job Parser"),
            ("rate(5 minutes)", "Cron Job Parser"),
            ("at(2026-12-25T09:00:00)", "Cron Job Parser"),
            ("<?xml version=\"1.0\"?><a/>", "XML Beautify/Minify"),
            ("<div><p>Hi</p></div>", "HTML Beautify/Minify"),
            ("select id, name from users where id = 1", "SQL Formatter"),
            ("hello%20world%21", "URL Encode/Decode"),
            ("Tom &amp; Jerry", "HTML Entity Encode/Decode"),
            ("48 65 6c 6c 6f", "Hex \u{2194} ASCII"),
            ("SGVsbG8sIHdvcmxkIQ==", "Base64 String Encode/Decode"),
            ("parseHttpResponse", "String Case Converter"),
            ("arn:aws:iam::123456789012:role/deploy", "AWS ARN Parser"),
            ("{\"awslogs\": {\"data\": \"H4sIAAAA\"}}", "CloudWatch Logs Decoder"),
            ("H4sIAAAAAAAC/8tIzcnJBwCGphA2BQAAAA==", "CloudWatch Logs Decoder"),
            ("apiVersion: v1\nkind: Pod\nmetadata:\n  name: web", "JSON \u{2194} YAML \u{2194} TOML"),
            ("# deps\n- serde\n- egui", "JSON \u{2194} YAML \u{2194} TOML"),
            ("[server]\nhost = \"localhost\"\nport = 8080", "JSON \u{2194} YAML \u{2194} TOML"),
            ("id,name\n1,Ada\n2,Grace", "JSON \u{2194} CSV"),
            ("id;name;city\n1;Ada;London\n2;Grace;\"New York\"", "JSON \u{2194} CSV"),
            ("user_id", "String Case Converter"),
        ];
        for (text, expected) in cases {
            assert_eq!(tool(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn leaves_plain_text_alone() {
        for text in ["", "   ", "hello world", "Hello", "abcdefgh", "12", "just a sentence, nothing more.",
            "Dear Ada,\nthanks for the notes.\nBest, Grace", "a,b\nc,d", "Note: call Ada", "x == y"] {
            assert_eq!(tool(text), None, "{text:?}");
        }
    }

    #[test]
    fn every_target_is_a_tool_that_accepts_input() {
        let tools = super::super::registry();
        let samples = ["eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln", "-----BEGIN CERTIFICATE-----", "data:image/png;base64,AA==", "{}",
            "https://a.io", "3f2504e0-4f89-11d3-9a0c-0305e82c3301", "10.0.0.0/16", "1700000000", "#fff", "0 * * * *", "<?xml?>", "<b>x</b>",
            "SELECT 1", "a%20b", "&amp;", "48656c6c6f", "SGVsbG8sIHdvcmxkIQ==", "fooBar", "arn:aws:s3:::bucket", "a,b\n1,2\n3,4", "H4sIAAAAAAAC/8tIzcnJBwCGphA2BQAAAA==", "a: 1\nb: 2", "a = 1", "cron(0 12 ? * MON-FRI *)"];
        for text in samples {
            let detected = detect(text).unwrap_or_else(|| panic!("{text} not recognized"));
            let tool = tools.iter().find(|t| t.name() == detected.tool).unwrap_or_else(|| panic!("no tool named {}", detected.tool));
            assert!(tool.accepts_input(), "{} does not accept input", detected.tool);
        }
    }
}
