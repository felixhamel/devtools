use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use super::{Category, TextTool, Tool};

pub fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    use std::io::Read;
    let mut bytes = [0; N];
    std::fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut bytes)).map_err(|e| e.to_string())?;
    Ok(bytes)
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
    Box::new(TextTool::new("String Case Converter", "aA", Category::Text, vec![("Inspect", convert_case)]))
}

fn convert_case(input: &str) -> Result<String, String> {
    let words: Vec<String> = input.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).map(|s| s.to_lowercase()).collect();
    let title: Vec<String> = words.iter().map(|w| { let mut c = w.chars(); c.next().map(|x| x.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }).collect();
    Ok(format!("lowercase: {}\nUPPERCASE: {}\nsnake_case: {}\nkebab-case: {}\nTitle Case: {}\ncamelCase: {}{}\nPascalCase: {}",
        input.to_lowercase(), input.to_uppercase(), words.join("_"), words.join("-"), title.join(" "),
        words.first().cloned().unwrap_or_default(), title.iter().skip(1).cloned().collect::<String>(), title.join("")))
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
        ("Generate", |_| Ok(URL_SAFE_NO_PAD.encode(random_bytes::<24>()?))),
    ]))
}
