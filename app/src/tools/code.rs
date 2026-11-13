use super::{Category, TextTool, Tool};

pub fn number_base() -> Box<dyn Tool> {
    Box::new(TextTool::new("Number Base Converter", "01", Category::Code, vec![("Inspect", number_bases)]))
}

fn number_bases(input: &str) -> Result<String, String> {
    let text = input.trim();
    let n = if let Some(v) = text.strip_prefix("0x") { i128::from_str_radix(v, 16) }
        else if let Some(v) = text.strip_prefix("0b") { i128::from_str_radix(v, 2) }
        else if let Some(v) = text.strip_prefix("0o") { i128::from_str_radix(v, 8) }
        else { text.parse() }
        .map_err(|_| "Enter an integer (prefix binary with 0b, octal with 0o, hex with 0x)".to_string())?;
    Ok(format!("Decimal: {n}\nBinary: {n:b}\nOctal: {n:o}\nHexadecimal: {n:X}"))
}
