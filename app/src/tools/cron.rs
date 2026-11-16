//! Cron Job Parser.
use super::{Category, TextTool, Tool};
use crate::ui::icon;

pub fn cron() -> Box<dyn Tool> {
    Box::new(TextTool::new("Cron Job Parser", icon::TIMER, Category::Date, vec![("Parse", |s| cron_report(s, chrono::Local::now()))])
        .describe("Enter a cron expression (5 fields, or 6 with seconds first), e.g. */15 9-17 * * MON-FRI").sample(r#"*/15 9-17 * * MON-FRI"#).key_value_output())
}

/// Describes the expression and lists its next 5 run times after `now`.
fn cron_report<Tz: chrono::TimeZone>(src: &str, now: chrono::DateTime<Tz>) -> Result<String, String>
where Tz::Offset: std::fmt::Display {
    let cron: croner::Cron = src.trim().parse().map_err(|e: croner::errors::CronError| e.to_string())?;
    let runs: Vec<String> = cron.iter_after(now).take(5).map(|t| format!("  {}", t.format("%a %Y-%m-%d %H:%M:%S"))).collect();
    Ok(format!("Description: {}\n\nNext {} runs (local time):\n{}", cron.describe(), runs.len(), runs.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn describes_and_lists_next_runs() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 10, 7, 0).unwrap();
        let out = cron_report("*/15 9-17 * * MON-FRI", now).unwrap();
        assert!(out.contains("Next 5 runs"), "{out}");
        // 2026-01-01 is a Thursday.
        assert!(out.contains("  Thu 2026-01-01 10:15:00\n  Thu 2026-01-01 10:30:00"), "{out}");
        assert!(cron_report("61 * * * *", now).is_err());
        assert!(cron_report("not cron", now).is_err());
    }
}
