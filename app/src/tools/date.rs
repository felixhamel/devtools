//! Date tools.
use super::{Category, TextTool, Tool};
use crate::ui::icon;

pub fn unix_time() -> Box<dyn Tool> {
    Box::new(TextTool::new("Unix Time Converter", icon::CLOCK, Category::Date, vec![("Inspect", unix_time_info)]).sample(r#"1700000000"#).key_value_output())
}

/// Splits a Unix timestamp (seconds) into days since the epoch and UTC time of day.
fn unix_time_info(input: &str) -> Result<String, String> {
    let n: i64 = input.trim().parse().map_err(|_| "Enter a Unix timestamp in seconds".to_string())?;
    Ok(format!("Unix timestamp: {n} seconds\nDays since 1970-01-01: {}\nTime (UTC): {:02}:{:02}:{:02}", n.div_euclid(86400), n.rem_euclid(86400)/3600, n.rem_euclid(3600)/60, n.rem_euclid(60)))
}
