//! Number Base Converter: an integer in decimal, hex, octal or binary, shown
//! in all four bases.
use super::{Category, TextTool, Tool};
use crate::ui::icon;

/// The Number Base Converter tool.
pub fn number_base() -> Box<dyn Tool> {
    Box::new(TextTool::new("Number Base Converter", icon::CALCULATOR, Category::Code, vec![("Convert", number_bases)])
        .describe("Enter an integer: decimal, or prefixed with 0x (hex), 0o (octal) or 0b (binary). Underscores and a leading - are allowed.").sample("0xFF").key_value_output())
}

/// Parses `[-](0x|0o|0b)?digits` (any case, `_` separators allowed) and prints it in the four bases.
fn number_bases(input: &str) -> Result<String, String> {
    let cleaned: String = input.chars().filter(|c| *c != '_' && !c.is_whitespace()).map(|c| c.to_ascii_lowercase()).collect();
    let (negative, body) = match cleaned.strip_prefix('-') { Some(rest) => (true, rest), None => (false, cleaned.as_str()) };
    let (radix, digits) = [("0x", 16), ("0o", 8), ("0b", 2)].iter()
        .find_map(|(prefix, radix)| body.strip_prefix(prefix).map(|d| (*radix, d)))
        .unwrap_or((10, body));
    let invalid = || "Enter an integer, e.g. 255, 0xFF, 0o377 or 0b1111_1111".to_string();
    // Reject a second sign: from_str_radix would otherwise accept "-+5" or "--5".
    if digits.is_empty() || digits.starts_with(['+', '-']) { return Err(invalid()); }
    let magnitude = u128::from_str_radix(digits, radix).map_err(|_| invalid())?;
    // Show negative numbers as a sign plus magnitude, not as 128-bit two's complement.
    let sign = if negative && magnitude != 0 { "-" } else { "" };
    Ok(format!("Decimal: {sign}{magnitude}\nHexadecimal: {sign}{magnitude:X}\nOctal: {sign}{magnitude:o}\nBinary: {sign}{magnitude:b}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_prefixes_signs_and_separators() {
        assert_eq!(number_bases("255").unwrap(), "Decimal: 255\nHexadecimal: FF\nOctal: 377\nBinary: 11111111");
        assert_eq!(number_bases("0XfF").unwrap(), number_bases("0b1111_1111").unwrap());
        assert!(number_bases("-0x10").unwrap().starts_with("Decimal: -16\nHexadecimal: -10\n"));
        assert!(number_bases("-5").unwrap().ends_with("Binary: -101"));
        assert!(number_bases("0x").is_err());
        assert!(number_bases("12a").is_err());
        assert!(number_bases("--5").is_err());
    }
}
