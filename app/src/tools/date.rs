//! Date tools.
use super::{Category, TextTool, Tool};

pub fn unix_time() -> Box<dyn Tool> {
    Box::new(TextTool::new("Unix Time Converter", "TM", Category::Date, vec![("Inspect", unix_time_info)]))
}

/// Splits a Unix timestamp (seconds) into days since the epoch and UTC time of day.
fn unix_time_info(input: &str) -> Result<String, String> {
    let n: i64 = input.trim().parse().map_err(|_| "Enter a Unix timestamp in seconds".to_string())?;
    Ok(format!("Unix timestamp: {n} seconds\nDays since 1970-01-01: {}\nTime (UTC): {:02}:{:02}:{:02}", n.div_euclid(86400), n.rem_euclid(86400)/3600, n.rem_euclid(3600)/60, n.rem_euclid(60)))
}
