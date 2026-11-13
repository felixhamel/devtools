//! Text tools and generators, plus helpers shared with other tools
//! (`random_bytes`, `words`).
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use super::{Category, TextTool, Tool};

/// `N` bytes from the operating system's secure random number generator.
pub fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|e| format!("Random generator unavailable: {e}"))?;
    Ok(bytes)
}

/// Splits an identifier or phrase into lowercase words. Handles `camelCase`,
/// `PascalCase`, `snake_case`, `kebab-case`, spaces and acronyms
/// (`HTTPServer` → `http`, `server`). Digits stay attached to the word before them.
pub fn words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !current.is_empty() { words.push(std::mem::take(&mut current)); }
            continue;
        }
        if c.is_uppercase() && !current.is_empty() {
            let prev = chars[i - 1];
            let next_is_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            // New word at "aB" (camel hump) or at the last capital of an acronym ("HTTPServer" → "HTTP" | "Server").
            if prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next_is_lower) {
                words.push(std::mem::take(&mut current));
            }
        }
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() { words.push(current); }
    words
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|first| first.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

pub fn lorem_ipsum() -> Box<dyn Tool> {
    Box::new(TextTool::new("Lorem Ipsum Generator", "LO", Category::Text, vec![("Generate", |_| Ok(
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\n\nUt enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".into()
    ))]))
}

pub fn string_inspector() -> Box<dyn Tool> {
    Box::new(TextTool::new("String Inspector", "Aa", Category::Text, vec![("Inspect", |s| Ok(
        format!("Characters: {}\nWords: {}\nLines: {}\nBytes: {}", s.chars().count(), s.split_whitespace().count(), s.lines().count(), s.len())
    ))]))
}

pub fn case_converter() -> Box<dyn Tool> {
    Box::new(TextTool::new("String Case Converter", "aA", Category::Text, vec![("Convert", convert_case)]))
}

fn convert_case(input: &str) -> Result<String, String> {
    let words = words(input);
    let title: Vec<String> = words.iter().map(|w| capitalize(w)).collect();
    let camel = words.first().cloned().unwrap_or_default() + &title.iter().skip(1).cloned().collect::<String>();
    Ok([
        format!("lowercase: {}", input.to_lowercase()),
        format!("UPPERCASE: {}", input.to_uppercase()),
        format!("camelCase: {camel}"),
        format!("PascalCase: {}", title.concat()),
        format!("snake_case: {}", words.join("_")),
        format!("CONSTANT_CASE: {}", words.join("_").to_uppercase()),
        format!("kebab-case: {}", words.join("-")),
        format!("Title Case: {}", title.join(" ")),
    ].join("\n"))
}

pub fn color() -> Box<dyn Tool> {
    Box::new(TextTool::new("Color Converter", "CL", Category::Text, vec![("Inspect", |input| {
        let h = input.trim().trim_start_matches('#');
        if h.len() != 6 || !h.is_ascii() { return Err("Enter a six-digit hex color, for example #56D28F".into()); }
        let n = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
        Ok(format!("HEX: #{n:06X}\nRGB: {}, {}, {}", n >> 16, (n >> 8) & 255, n & 255))
    })]))
}

pub fn random_string() -> Box<dyn Tool> {
    Box::new(TextTool::new("Random String Generator", "R?", Category::Text, vec![
        // 24 random bytes → 32 URL-safe characters.
        ("Generate", |_| Ok(URL_SAFE_NO_PAD.encode(random_bytes::<24>()?))),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words() {
        assert_eq!(words("helloWorld"), ["hello", "world"]);
        assert_eq!(words("HTTPServer_id-2x"), ["http", "server", "id", "2x"]);
        assert_eq!(words("user ID v2Beta"), ["user", "id", "v2", "beta"]);
        assert!(words(" -_ ").is_empty());
    }

    #[test]
    fn converts_camel_case_input() {
        let out = convert_case("parseHTTPResponse").unwrap();
        assert!(out.contains("snake_case: parse_http_response"), "{out}");
        assert!(out.contains("camelCase: parseHttpResponse"), "{out}");
        assert!(out.contains("CONSTANT_CASE: PARSE_HTTP_RESPONSE"), "{out}");
        assert!(out.contains("Title Case: Parse Http Response"), "{out}");
    }

    #[test]
    fn random_bytes_differ() {
        assert_ne!(random_bytes::<16>().unwrap(), random_bytes::<16>().unwrap());
    }
}
