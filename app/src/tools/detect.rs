//! Recognizes what a pasted text is (a JWT, JSON, a timestamp, a URL…) to open
//! it in the right tool: ⌘V on the home screen and the palette's "Paste" command.
use regex::Regex;
use std::sync::LazyLock;

/// A recognized kind of text: the tool that handles it, and a short name for it.
#[derive(Debug, PartialEq, Eq)]
pub struct Detected { pub tool: &'static str, pub what: &'static str }

fn re(pattern: &str) -> Regex { Regex::new(pattern).expect("detection patterns are valid") }

static JWT: LazyLock<Regex> = LazyLock::new(|| re(r"^eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*$"));
static UUID: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(urn:uuid:)?\{?[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}\}?$"));
static ULID: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^[0-7][0-9A-HJKMNP-TV-Z]{25}$"));
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| re(r"^\d{9,19}(\.\d+)?$"));
static DATE: LazyLock<Regex> = LazyLock::new(|| re(r"^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}.*)?$"));
static COLOR: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})|(rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(.+\))$"));
static CRON_FIELD: LazyLock<Regex> = LazyLock::new(|| re(r"^[0-9*/,?LW#A-Za-z-]+$"));
static PERCENT: LazyLock<Regex> = LazyLock::new(|| re(r"%[0-9A-Fa-f]{2}"));
static ENTITY: LazyLock<Regex> = LazyLock::new(|| re(r"&(#\d+|#x[0-9A-Fa-f]+|[A-Za-z]+);"));
static HEX: LazyLock<Regex> = LazyLock::new(|| re(r"^([0-9A-Fa-f]{2}[\s:]?){4,}$"));
static BASE64: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z0-9+/_-]{8,}={0,2}$"));
static IDENTIFIER: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z][A-Za-z0-9]*([_.-][A-Za-z0-9]+)+$|^[a-z]+([A-Z][a-z0-9]*)+$|^([A-Z][a-z0-9]+){2,}$"));

const SQL_KEYWORDS: [&str; 8] = ["SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "DROP", "WITH"];

/// The tool that best handles `text`, or `None` when nothing specific is recognized.
pub fn detect(text: &str) -> Option<Detected> {
    let t = text.trim();
    let found = |tool, what| Some(Detected { tool, what });
    let one_word = !t.is_empty() && !t.contains(char::is_whitespace);
    if t.is_empty() { return None; }
    if JWT.is_match(t) { return found("JWT Debugger", "a JWT"); }
    if t.contains("-----BEGIN CERTIFICATE-----") { return found("Certificate Decoder (X.509)", "a certificate"); }
    if t.starts_with("data:image/") { return found("Base64 Image Encode/Decode", "an image data URI"); }
    if (t.starts_with('{') || t.starts_with('[')) && serde_json::from_str::<serde_json::Value>(t).is_ok() { return found("JSON Format/Validate", "JSON"); }
    if one_word && t.contains("://") && url::Url::parse(t).is_ok_and(|u| u.has_host()) { return found("URL Parser", "a URL"); }
    if UUID.is_match(t) { return found("UUID/ULID Generate/Decode", "a UUID"); }
    if one_word && (t.parse::<ipnet::IpNet>().is_ok() || t.parse::<std::net::IpAddr>().is_ok()) {
        return found("IP Subnet Calculator", if t.contains('/') { "a CIDR block" } else { "an IP address" });
    }
    if ULID.is_match(t) && t.chars().any(|c| c.is_ascii_digit()) { return found("UUID/ULID Generate/Decode", "a ULID"); }
    if TIMESTAMP.is_match(t) { return found("Unix Time Converter", "a timestamp"); }
    if DATE.is_match(t) { return found("Unix Time Converter", "a date"); }
    if COLOR.is_match(t) { return found("Color Converter", "a color"); }
    let fields: Vec<&str> = t.split_whitespace().collect();
    if (5..=7).contains(&fields.len()) && fields.iter().all(|f| CRON_FIELD.is_match(f)) && fields.iter().any(|f| f.contains(['*', '/']))
        && t.parse::<croner::Cron>().is_ok() {
        return found("Cron Job Parser", "a cron expression");
    }
    if t.starts_with("<?xml") { return found("XML Beautify/Minify", "XML"); }
    if t.starts_with('<') && t.ends_with('>') { return found("HTML Beautify/Minify", "HTML"); }
    let first = fields.first().map(|w| w.to_ascii_uppercase()).unwrap_or_default();
    if SQL_KEYWORDS.contains(&first.as_str()) && fields.len() > 1 { return found("SQL Formatter", "SQL"); }
    if one_word && PERCENT.is_match(t) { return found("URL Encode/Decode", "URL-encoded text"); }
    if ENTITY.is_match(t) { return found("HTML Entity Encode/Decode", "HTML entities"); }
    if HEX.is_match(t) && t.chars().any(|c| c.is_ascii_digit()) && t.chars().any(|c| c.is_ascii_alphabetic()) { return found("Hex \u{2194} ASCII", "hex bytes"); }
    if BASE64.is_match(t) && t.len().is_multiple_of(4) && decodes_to_text(t) { return found("Base64 String Encode/Decode", "Base64"); }
    if IDENTIFIER.is_match(t) { return found("String Case Converter", "an identifier"); }
    None
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
            ("<?xml version=\"1.0\"?><a/>", "XML Beautify/Minify"),
            ("<div><p>Hi</p></div>", "HTML Beautify/Minify"),
            ("select id, name from users where id = 1", "SQL Formatter"),
            ("hello%20world%21", "URL Encode/Decode"),
            ("Tom &amp; Jerry", "HTML Entity Encode/Decode"),
            ("48 65 6c 6c 6f", "Hex \u{2194} ASCII"),
            ("SGVsbG8sIHdvcmxkIQ==", "Base64 String Encode/Decode"),
            ("parseHttpResponse", "String Case Converter"),
            ("user_id", "String Case Converter"),
        ];
        for (text, expected) in cases {
            assert_eq!(tool(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn leaves_plain_text_alone() {
        for text in ["", "   ", "hello world", "Hello", "abcdefgh", "12", "just a sentence, nothing more."] {
            assert_eq!(tool(text), None, "{text:?}");
        }
    }

    #[test]
    fn every_target_is_a_tool_that_accepts_input() {
        let tools = super::super::registry();
        let samples = ["eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln", "-----BEGIN CERTIFICATE-----", "data:image/png;base64,AA==", "{}",
            "https://a.io", "3f2504e0-4f89-11d3-9a0c-0305e82c3301", "10.0.0.0/16", "1700000000", "#fff", "0 * * * *", "<?xml?>", "<b>x</b>",
            "SELECT 1", "a%20b", "&amp;", "48656c6c6f", "SGVsbG8sIHdvcmxkIQ==", "fooBar"];
        for text in samples {
            let detected = detect(text).unwrap_or_else(|| panic!("{text} not recognized"));
            let tool = tools.iter().find(|t| t.name() == detected.tool).unwrap_or_else(|| panic!("no tool named {}", detected.tool));
            assert!(tool.accepts_input(), "{} does not accept input", detected.tool);
        }
    }
}
