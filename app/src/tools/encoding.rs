//! Text encoders and decoders built as [`TextTool`]s (URL, HTML entities,
//! backslash escapes, hex), and the lenient Base64 decoder other tools share.
use std::fmt::Write;
use base64::{alphabet, engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig}, Engine};
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

/// URL Encode/Decode: percent-encoding as a component or as a full URL.
pub fn url() -> Box<dyn Tool> {
    Box::new(TextTool::new("URL Encode/Decode", icon::PERCENT, Category::Encoding, vec![
        ("Component", |s| Ok(percent_encoding::utf8_percent_encode(s, COMPONENT).to_string())),
        ("Full URL", |s| Ok(percent_encoding::utf8_percent_encode(s, FULL_URL).to_string())),
        ("Decode", url_decode),
    ])
    .describe("Component escapes everything but letters, digits and - _ . ~ (encodeURIComponent). Full URL keeps : / ? # & = intact (encodeURI).")
    .sample("https://example.com/search?q=café & crème&lang=fr#résultats"))
}

/// Percent-encodes all but RFC 3986 unreserved characters, like `encodeURIComponent`.
pub(super) const COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

/// Also keeps the characters that give a URL its structure, like `encodeURI`.
const FULL_URL: &percent_encoding::AsciiSet = &COMPONENT
    .remove(b';').remove(b',').remove(b'/').remove(b'?').remove(b':').remove(b'@').remove(b'&').remove(b'=').remove(b'+')
    .remove(b'$').remove(b'!').remove(b'*').remove(b'\'').remove(b'(').remove(b')').remove(b'#');

/// Decodes `%XX` sequences; the result must be UTF-8. `+` is kept as is
/// (it only means a space in form data).
fn url_decode(input: &str) -> Result<String, String> {
    percent_encoding::percent_decode_str(input).decode_utf8().map(std::borrow::Cow::into_owned)
        .map_err(|_| "The decoded bytes are not valid UTF-8 text".to_string())
}

/// HTML Entity Encode/Decode: escapes the five characters special in HTML,
/// or decodes named and numeric entities.
pub fn html_entity() -> Box<dyn Tool> {
    Box::new(TextTool::new("HTML Entity Encode/Decode", icon::BRACKETS_ANGLE, Category::Encoding, vec![
        ("Encode", |s| Ok(s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;"))),
        // Every named entity (&eacute;, &nbsp;…) and numeric ones (&#169;, &#x27;).
        ("Decode", |s| Ok(html_escape::decode_html_entities(s).into_owned())),
    ])
    .describe("Encode turns & < > \" ' into entities; Decode understands every named entity (&eacute;, &nbsp;…) and numeric ones.")
    .sample(r#"<a href="/menu">Café & "thé"</a>"#).syntax("html"))
}

/// Backslash Escape/Unescape, for string literals in JSON, JavaScript, C…
pub fn backslash() -> Box<dyn Tool> {
    Box::new(TextTool::new("Backslash Escape/Unescape", icon::QUOTES, Category::Encoding, vec![("Escape", escape), ("Unescape", unescape)])
        .describe("Escape special characters for a string literal, or turn \\n, \\t, \\uXXXX… back into characters.").sample(r#"Line 1
	Say "hi"
C:\Users"#))
}

/// Escapes `\`, `"`, line breaks, tabs and other control characters (as
/// `\uXXXX`), so the text fits in a double-quoted JSON or JavaScript string.
fn escape(input: &str) -> Result<String, String> {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => { let _ = write!(out, "\\u{:04x}", u32::from(c)); }
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
        // The decoded character (`None` if malformed) and how many input characters it took.
        let (decoded, used) = match chars[i + 1] {
            'n' => (Some('\n'), 2), 'r' => (Some('\r'), 2), 't' => (Some('\t'), 2),
            'b' => (Some('\u{8}'), 2), 'f' => (Some('\u{c}'), 2), 'v' => (Some('\u{b}'), 2), '0' => (Some('\0'), 2),
            c @ ('\\' | '"' | '\'' | '/') => (Some(c), 2),
            'x' => match hex(i + 2, 2) { Some(c) => (char::from_u32(c), 4), None => (None, 2) },
            // `\u{1F600}`: any number of hex digits between braces (Rust, ES2015).
            'u' if chars.get(i + 2) == Some(&'{') => {
                let end = chars[i + 3..].iter().position(|&c| c == '}').map(|k| i + 3 + k);
                match end.and_then(|end| hex(i + 3, end - i - 3).map(|c| (c, end))) {
                    Some((c, end)) => (char::from_u32(c), end - i + 1),
                    // Not `\u{hex}`: keep the backslash and `u`, read on from there.
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

/// Hex ↔ ASCII: text to hexadecimal UTF-8 bytes and back.
pub fn hex() -> Box<dyn Tool> {
    Box::new(TextTool::new("Hex \u{2194} ASCII", icon::HEXAGON, Category::Encoding, vec![("Text \u{2192} Hex", text_to_hex), ("Hex \u{2192} Text", hex_to_text)])
        .describe("Converts text to hexadecimal bytes and back. Hex input may use spaces, colons, 0x or \\x prefixes.")
        .sample("Hello, world!"))
}

/// Each UTF-8 byte as two uppercase hex digits, separated by spaces.
fn text_to_hex(input: &str) -> Result<String, String> {
    Ok(input.as_bytes().iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "))
}

/// Accepts `48656C6C6F`, `48 65 6c`, `48:65`, `48,65`, `48-65`, `0x48 0x65` or
/// `\x48\x65`. The bytes must be UTF-8 text.
fn hex_to_text(input: &str) -> Result<String, String> {
    let lower = input.to_ascii_lowercase().replace("0x", "").replace("\\x", "");
    let digits: String = lower.chars().filter(|c| !c.is_whitespace() && !matches!(c, ':' | ',' | '-')).collect();
    if !digits.len().is_multiple_of(2) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Enter pairs of hexadecimal digits".into());
    }
    let bytes: Vec<u8> = (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(&digits[i..i + 2], 16).expect("checked hex digits")).collect();
    String::from_utf8(bytes).map_err(|_| "The bytes are not valid UTF-8 text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encoding_modes() {
        let component = |s| percent_encoding::utf8_percent_encode(s, COMPONENT).to_string();
        let full = |s| percent_encoding::utf8_percent_encode(s, FULL_URL).to_string();
        assert_eq!(component("a b&c/é~"), "a%20b%26c%2F%C3%A9~");
        assert_eq!(full("https://x.com/a b?q=é&r=1#top"), "https://x.com/a%20b?q=%C3%A9&r=1#top");
        assert_eq!(url_decode("a%20b%26c%2F%C3%A9").unwrap(), "a b&c/é");
        assert!(url_decode("%FF").is_err());
    }

    #[test]
    fn hex_round_trip_and_formats() {
        assert_eq!(text_to_hex("Hé!").unwrap(), "48 C3 A9 21");
        assert_eq!(hex_to_text("48 C3 A9 21").unwrap(), "Hé!");
        assert_eq!(hex_to_text("0x48 0x69").unwrap(), "Hi");
        assert_eq!(hex_to_text(r"\x48\x69").unwrap(), "Hi");
        assert_eq!(hex_to_text("48:69").unwrap(), "Hi");
        assert!(hex_to_text("4").is_err());
        assert!(hex_to_text("zz").is_err());
        assert!(hex_to_text("ff").is_err()); // not UTF-8
    }

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
        // A malformed escape must not swallow the next one.
        assert_eq!(unescape(r"\x4\n").unwrap(), "\\x4\n");
        assert_eq!(unescape(r"\u{zz}\t}").unwrap(), "\\u{zz}\t}");
    }
}
