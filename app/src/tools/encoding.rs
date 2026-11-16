//! Encoders and decoders: Base64, URL, HTML entities, backslash escapes, hex.
use base64::{alphabet, engine::{general_purpose::STANDARD, DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig}, Engine};
use super::{Category, TextTool, Tool};
use crate::ui::icon;

/// Standard alphabet that accepts input with or without `=` padding.
const LENIENT: GeneralPurpose = GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent));

/// Decodes Base64 the way people paste it: whitespace and line breaks are
/// ignored, padding is optional and the URL-safe alphabet (`-`, `_`) is accepted.
pub fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    let normalized: String = text.chars().filter(|c| !c.is_whitespace())
        .map(|c| match c { '-' => '+', '_' => '/', c => c }).collect();
    LENIENT.decode(normalized).map_err(|e| format!("Invalid Base64: {e}"))
}

pub fn base64_string() -> Box<dyn Tool> {
    Box::new(TextTool::new("Base64 String Encode/Decode", icon::BINARY, Category::Encoding, vec![
        ("Encode", |s| Ok(STANDARD.encode(s.as_bytes()))),
        ("Decode", |s| String::from_utf8(decode_base64(s)?).map_err(|_| "Decoded bytes are not UTF-8 text".to_string())),
    ]).sample(r#"Hello, DevUtils! 👋"#))
}

pub fn url() -> Box<dyn Tool> {
    Box::new(TextTool::new("URL Encode/Decode", icon::PERCENT, Category::Encoding, vec![
        ("Encode", |s| Ok(urlencoding::encode(s).into_owned())),
        ("Decode", |s| urlencoding::decode(s).map(|s| s.into_owned()).map_err(|e| e.to_string())),
    ]).sample(r#"name=Jean Dupont&city=Montréal & co"#))
}

pub fn html_entity() -> Box<dyn Tool> {
    Box::new(TextTool::new("HTML Entity Encode/Decode", icon::BRACKETS_ANGLE, Category::Encoding, vec![
        ("Encode", |s| Ok(s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;"))),
        // Every named entity (&eacute;, &nbsp;…) and numeric ones (&#169;, &#x27;).
        ("Decode", |s| Ok(html_escape::decode_html_entities(s).into_owned())),
    ]).sample(r#"<a href="/menu">Café & "thé"</a>"#).syntax("html"))
}

pub fn backslash() -> Box<dyn Tool> {
    Box::new(TextTool::new("Backslash Escape/Unescape", icon::QUOTES, Category::Encoding, vec![("Escape", escape), ("Unescape", unescape)])
        .describe("Escape special characters for a string literal, or turn \\n, \\t, \\uXXXX… back into characters.").sample(r#"Line 1
	Say "hi"
C:\Users"#))
}

fn escape(input: &str) -> Result<String, String> {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    Ok(out)
}

/// Reverses C/JSON/JavaScript-style escapes: `\n \r \t \b \f \v \0 \\ \" \' \/`,
/// `\xHH`, `\uXXXX` (including surrogate pairs) and `\u{…}`. Unknown or
/// malformed escapes are kept as written.
fn unescape(input: &str) -> Result<String, String> {
    let chars: Vec<char> = input.chars().collect();
    // Exactly `len` hex digits starting at `from` (from_str_radix alone would also accept a leading `+`).
    let hex = |from: usize, len: usize| -> Option<u32> {
        let digits = chars.get(from..from + len)?;
        if digits.is_empty() || !digits.iter().all(char::is_ascii_hexdigit) { return None; }
        u32::from_str_radix(&digits.iter().collect::<String>(), 16).ok()
    };
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '\\' || i + 1 == chars.len() { out.push(chars[i]); i += 1; continue; }
        let (decoded, used) = match chars[i + 1] {
            'n' => (Some('\n'), 2), 'r' => (Some('\r'), 2), 't' => (Some('\t'), 2),
            'b' => (Some('\u{8}'), 2), 'f' => (Some('\u{c}'), 2), 'v' => (Some('\u{b}'), 2), '0' => (Some('\0'), 2),
            c @ ('\\' | '"' | '\'' | '/') => (Some(c), 2),
            'x' => (hex(i + 2, 2).and_then(char::from_u32), 4),
            'u' if chars.get(i + 2) == Some(&'{') => {
                let end = chars[i + 3..].iter().position(|&c| c == '}').map(|k| i + 3 + k);
                match end {
                    Some(end) => (hex(i + 3, end - i - 3).and_then(char::from_u32), end - i + 1),
                    None => (None, 2),
                }
            }
            'u' => match hex(i + 2, 4) {
                // A high surrogate must be followed by `\uDC00`–`\uDFFF` to form one character.
                Some(high @ 0xD800..=0xDBFF) if chars.get(i + 6) == Some(&'\\') && chars.get(i + 7) == Some(&'u') => {
                    match hex(i + 8, 4) {
                        Some(low @ 0xDC00..=0xDFFF) => (char::from_u32(0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)), 12),
                        _ => (None, 6),
                    }
                }
                Some(code) => (char::from_u32(code), 6),
                None => (None, 2),
            },
            _ => (None, 2),
        };
        match decoded {
            Some(c) => out.push(c),
            None => out.extend(&chars[i..(i + used).min(chars.len())]),
        }
        i += used;
    }
    Ok(out)
}

pub fn hex_to_ascii() -> Box<dyn Tool> {
    Box::new(TextTool::new("Hex to ASCII", icon::HEXAGON, Category::Encoding, vec![("Convert", |input| {
        let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        let compact = compact.strip_prefix("0x").unwrap_or(&compact);
        if !compact.len().is_multiple_of(2) || !compact.is_ascii() { return Err("Enter pairs of hexadecimal digits".into()); }
        let bytes: Result<Vec<u8>, _> = (0..compact.len()).step_by(2).map(|i| u8::from_str_radix(&compact[i..i + 2], 16)).collect();
        String::from_utf8(bytes.map_err(|e| e.to_string())?).map_err(|_| "The bytes are not valid UTF-8 text".to_string())
    })]).sample(r#"48 65 6C 6C 6F 2C 20 77 6F 72 6C 64 21"#))
}

pub fn ascii_to_hex() -> Box<dyn Tool> {
    Box::new(TextTool::new("ASCII to Hex", icon::TEXT_T, Category::Encoding, vec![
        ("Convert", |s| Ok(s.as_bytes().iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "))),
    ]).sample(r#"Hello, world!"#))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_lenient() {
        assert_eq!(decode_base64("aGVsbG8gd29y\nbGQ=").unwrap(), b"hello world");
        assert_eq!(decode_base64("aGVsbG8gd29ybGQ").unwrap(), b"hello world");
        assert_eq!(decode_base64("-_8").unwrap(), [0xfb, 0xff]);
        assert!(decode_base64("a").is_err());
    }

    #[test]
    fn decodes_all_html_entities() {
        let decode = |s: &str| html_escape::decode_html_entities(s).into_owned();
        assert_eq!(decode("&eacute;t&eacute;&nbsp;&#169; &#x27;a&#39; &amp;lt;"), "été\u{a0}© 'a' &lt;");
    }

    #[test]
    fn escape_round_trip() {
        let text = "a\"b\\c\nd\te\u{1}é";
        assert_eq!(escape(text).unwrap(), "a\\\"b\\\\c\\nd\\te\\u0001é");
        assert_eq!(unescape(&escape(text).unwrap()).unwrap(), text);
    }

    #[test]
    fn unescapes_unicode_and_keeps_unknown() {
        assert_eq!(unescape(r"é\x41\u{1F600}😀\'\/").unwrap(), "éA😀😀'/");
        assert_eq!(unescape(r"\q \u12 \x+1 \u{} \").unwrap(), r"\q \u12 \x+1 \u{} \");
    }
}
