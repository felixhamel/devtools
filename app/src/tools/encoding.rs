use base64::{engine::general_purpose::STANDARD, Engine};
use super::{Category, TextTool, Tool};

pub fn base64_string() -> Box<dyn Tool> {
    Box::new(TextTool::new("Base64 String Encode/Decode", "64", Category::Encoding, vec![
        ("Encode", |s| Ok(STANDARD.encode(s.as_bytes()))),
        ("Decode", |s| STANDARD.decode(s.trim()).map_err(|e| e.to_string()).and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))),
    ]))
}

pub fn url() -> Box<dyn Tool> {
    Box::new(TextTool::new("URL Encode/Decode", "%", Category::Encoding, vec![
        ("Encode", |s| Ok(urlencoding::encode(s).into_owned())),
        ("Decode", |s| urlencoding::decode(s).map(|s| s.into_owned()).map_err(|e| e.to_string())),
    ]))
}

pub fn html_entity() -> Box<dyn Tool> {
    Box::new(TextTool::new("HTML Entity Encode/Decode", "&;", Category::Encoding, vec![
        ("Encode", |s| Ok(s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;"))),
        ("Decode", |s| Ok(s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&"))),
    ]))
}

pub fn backslash() -> Box<dyn Tool> {
    Box::new(TextTool::new("Backslash Escape/Unescape", "\\", Category::Encoding, vec![
        ("Encode", |s| Ok(s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t").replace('"', "\\\""))),
        ("Decode", unescape),
    ]))
}

fn unescape(input: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '\\' { out.push(c); continue; }
        match chars.next() {
            Some('n') => out.push('\n'), Some('r') => out.push('\r'), Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'), Some('"') => out.push('"'),
            Some(x) => { out.push('\\'); out.push(x) }
            None => out.push('\\'),
        }
    }
    Ok(out)
}

pub fn hex_to_ascii() -> Box<dyn Tool> {
    Box::new(TextTool::new("Hex to ASCII", "HX", Category::Encoding, vec![("Inspect", |input| {
        let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.len() % 2 != 0 || !compact.is_ascii() { return Err("Enter pairs of hexadecimal digits".into()); }
        let bytes: Result<Vec<u8>, _> = (0..compact.len()).step_by(2).map(|i| u8::from_str_radix(&compact[i..i+2], 16)).collect();
        String::from_utf8(bytes.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    })]))
}

pub fn ascii_to_hex() -> Box<dyn Tool> {
    Box::new(TextTool::new("ASCII to Hex", "AH", Category::Encoding, vec![
        ("Inspect", |s| Ok(s.as_bytes().iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "))),
    ]))
}
