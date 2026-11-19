//! Cron Job Parser: describes a cron expression in words and lists its next
//! run times (parsing and scheduling by `croner`). Also reads AWS EventBridge
//! schedules: `cron(…)` with its own field order, `rate(…)` and `at(…)`.
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use super::{Category, TextTool, Tool};
use crate::ui::icon;

/// How many upcoming runs the report lists.
const RUN_COUNT: usize = 5;

/// Shown when an EventBridge `cron(…)` does not have its 6 fields.
const EVENTBRIDGE_FIELDS: &str = "An EventBridge cron has 6 fields: minutes hours day-of-month month day-of-week year, e.g. cron(0 12 ? * MON-FRI *)";

/// Shown when a `rate(…)` cannot be read.
const RATE_FORMAT: &str = "A rate is rate(value unit), e.g. rate(5 minutes), rate(1 hour) or rate(7 days)";

/// The Cron Job Parser tool. Standard cron runs are computed in local time,
/// EventBridge ones in UTC (as EventBridge rules run).
pub fn cron() -> Box<dyn Tool> {
    Box::new(TextTool::new("Cron Job Parser", icon::TIMER, Category::Date, vec![("Parse", |s| report(s, Utc::now()))])
        .describe("A cron expression (5 fields, or 6 with seconds first), or an AWS EventBridge schedule: cron(0 12 ? * MON-FRI *), rate(5 minutes) or at(…).")
        .sample("*/15 9-17 * * MON-FRI").key_value_output())
}

/// The report for `src`, whichever syntax it uses.
fn report(src: &str, now: DateTime<Utc>) -> Result<String, String> {
    let t = src.trim();
    if let Some(fields) = wrapped(t, "cron") { return eventbridge_report(fields, now); }
    if let Some(rate) = wrapped(t, "rate") { return rate_report(rate); }
    if let Some(at) = wrapped(t, "at") { return at_report(at, now); }
    // Without "cron(…)": a `?` where standard cron has its hour or month can
    // only be EventBridge's order (day of month or day of week there).
    let fields: Vec<&str> = t.split_whitespace().collect();
    if fields.len() == 6 && (fields[2] == "?" || fields[4] == "?") { return eventbridge_report(t, now); }
    cron_report(t, now.with_timezone(&Local))
}

/// The text inside `name(…)`, e.g. `wrapped("rate(5 minutes)", "rate")` is `Some("5 minutes")`.
fn wrapped<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?.trim_start().strip_prefix('(')?.strip_suffix(')').map(str::trim)
}

/// Describes the expression and lists its next [`RUN_COUNT`] run times after
/// `now`, in `now`'s time zone (generic so the tests can use UTC).
fn cron_report<Tz: chrono::TimeZone>(src: &str, now: chrono::DateTime<Tz>) -> Result<String, String>
where Tz::Offset: std::fmt::Display {
    let cron: croner::Cron = src.trim().parse().map_err(|e: croner::errors::CronError| e.to_string())?;
    let runs: Vec<String> = cron.iter_after(now).take(RUN_COUNT).map(|t| format!("  {}", t.format("%a %Y-%m-%d %H:%M:%S"))).collect();
    Ok(format!("Description: {}\n\nNext {} runs (local time):\n{}", cron.describe(), runs.len(), runs.join("\n")))
}

/// An EventBridge cron (`minutes hours day-of-month month day-of-week year`,
/// days of the week 1-7 from Sunday, `?` in one of the two day fields) and its
/// next runs in UTC, with the local time of each.
fn eventbridge_report(expression: &str, now: DateTime<Utc>) -> Result<String, String> {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    let [minutes, hours, day, month, weekday, year] = fields.as_slice() else { return Err(EVENTBRIDGE_FIELDS.into()) };
    if (*day == "?") == (*weekday == "?") {
        return Err("EventBridge needs ? in exactly one of day-of-month and day-of-week, e.g. cron(0 12 ? * MON-FRI *)".into());
    }
    // croner reads 7 fields with seconds first. Its Quartz mode numbers the days
    // as EventBridge does, and EventBridge writes steps as `0/15` (from 0, every 15).
    let cron = croner::parser::CronParser::builder().alternative_weekdays(true).sloppy_ranges(true).build()
        .parse(&format!("0 {minutes} {hours} {day} {month} {weekday} {year}")).map_err(|e| e.to_string())?;
    let runs: Vec<String> = cron.iter_after(now).take(RUN_COUNT)
        .map(|t| format!("  {} UTC ({} local)", t.format("%a %Y-%m-%d %H:%M"), t.with_timezone(&Local).format("%a %H:%M"))).collect();
    let next = if runs.is_empty() { "Next runs: none (the schedule has no date after today)".to_owned() } else { format!("Next {} runs (UTC):\n{}", runs.len(), runs.join("\n")) };
    Ok(format!("Syntax: EventBridge cron (times in UTC)\nDescription: {}\n\n{next}", cron.describe()))
}

/// A `rate(value unit)`: EventBridge wants the unit singular for 1 and plural otherwise.
fn rate_report(rate: &str) -> Result<String, String> {
    let (value, unit) = rate.split_once(' ').ok_or(RATE_FORMAT)?;
    let value: u64 = value.parse().ok().filter(|v| *v > 0).ok_or("The value must be a whole number above 0, e.g. rate(5 minutes)")?;
    let (singular, seconds) = match unit.trim().trim_end_matches('s') {
        "minute" => ("minute", 60),
        "hour" => ("hour", 3_600),
        "day" => ("day", 86_400),
        _ => return Err(RATE_FORMAT.into()),
    };
    let expected = if value == 1 { singular.to_owned() } else { format!("{singular}s") };
    if unit.trim() != expected { return Err(format!("EventBridge expects rate({value} {expected})")); }
    let interval = value.checked_mul(seconds).ok_or("That rate is too large")?;
    let mut lines = vec![
        "Syntax: EventBridge rate".to_owned(),
        format!("Description: {}", if value == 1 { format!("Every {singular}") } else { format!("Every {value} {expected}") }),
        format!("Interval: {interval} seconds"),
    ];
    if interval <= 86_400 {
        let per_day = 86_400.0 / interval as f64;
        lines.push(format!("Runs per day: {}", if per_day.fract() == 0.0 { format!("{per_day}") } else { format!("about {per_day:.0}") }));
    }
    lines.push("Start: counted from when the rule or schedule is created".to_owned());
    Ok(lines.join("\n"))
}

/// An EventBridge Scheduler one-time schedule, `at(yyyy-mm-ddThh:mm:ss)`, in
/// the schedule's time zone (UTC unless one is set).
fn at_report(at: &str, now: DateTime<Utc>) -> Result<String, String> {
    let when = NaiveDateTime::parse_from_str(at, "%Y-%m-%dT%H:%M:%S").map_err(|_| "A one-time schedule is at(yyyy-mm-ddThh:mm:ss), e.g. at(2026-12-25T09:00:00)")?;
    Ok(format!("Syntax: EventBridge Scheduler one-time schedule\nRuns once: {}\nTime zone: the schedule's (UTC unless one is set)\nRelative (if UTC): {}",
        when.format(super::unix_time::DATE_TIME_FORMAT), super::unix_time::relative(when.and_utc() - now)))
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

    #[test]
    fn reads_eventbridge_cron() {
        // Thursday 1 Jan 2026, 10:07 UTC.
        let now = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 10, 7, 0).unwrap();
        let out = report("cron(0 12 ? * MON-FRI *)", now).unwrap();
        assert!(out.starts_with("Syntax: EventBridge cron (times in UTC)\nDescription: "), "{out}");
        assert!(out.contains("Next 5 runs (UTC):\n  Thu 2026-01-01 12:00 UTC ("), "{out}");
        assert!(out.contains("  Fri 2026-01-02 12:00 UTC ("), "{out}");
        // 1 is Sunday, as in EventBridge; the wrapper is optional.
        assert!(report("30 8 ? * 1 *", now).unwrap().contains("  Sun 2026-01-04 08:30 UTC ("));
        // The year field, and the last day of the month.
        assert!(report("cron(0 0 L * ? 2027)", now).unwrap().contains("  Sun 2027-01-31 00:00 UTC ("));
        assert!(report("cron(0 0 1 1 ? 2020)", now).unwrap().contains("Next runs: none"));
        // AWS's own "every 15 minutes" example.
        assert!(report("cron(0/15 * * * ? *)", now).unwrap().contains("  Thu 2026-01-01 10:15 UTC ("));
        assert!(report("cron(0 12 * * MON *)", now).unwrap_err().contains("exactly one"));
        assert!(report("cron(0 12 * *)", now).unwrap_err().contains("6 fields"));
    }

    #[test]
    fn follows_eventbridge_rules() {
        // Thursday 1 Jan 2026, 10:07 UTC.
        let now = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 10, 7, 0).unwrap();
        let first_run = |expression: &str| report(expression, now).unwrap().lines().find(|l| l.starts_with("  ")).unwrap_or_default().to_owned();
        // 7 is Saturday; 2#1 the first Monday; 15W the weekday nearest the 15th; names in any case.
        assert!(first_run("cron(0 8 ? * 7 *)").starts_with("  Sat 2026-01-03 08:00 UTC"));
        assert!(first_run("cron(0 9 ? * 2#1 *)").starts_with("  Mon 2026-01-05 09:00 UTC"));
        assert!(first_run("cron(0 12 ? * mon-fri *)").starts_with("  Thu 2026-01-01 12:00 UTC"));
        // 15 Feb 2026 is a Sunday: the nearest weekday is Monday the 16th.
        assert!(report("cron(0 9 15W * ? *)", now).unwrap().contains("  Mon 2026-02-16 09:00 UTC"));
        assert!(report("cron(61 * ? * * *)", now).is_err());
    }

    #[test]
    fn keeps_standard_cron_apart() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 10, 7, 0).unwrap();
        // 5 fields, and 6 with seconds first, are read as before, in local time.
        assert!(report("0 0 * * *", now).unwrap().contains("(local time)"));
        assert!(report("*/30 * * * * *", now).unwrap().contains("(local time)"));
        // Without its wrapper, a `?` in the day-of-week position marks EventBridge.
        assert!(report("0 18 L * ? *", now).unwrap().starts_with("Syntax: EventBridge cron"));
        assert_eq!(wrapped("rate( 5 minutes )", "rate"), Some("5 minutes"));
        assert_eq!(wrapped("rate 5 minutes", "rate"), None);
        assert_eq!(wrapped("cron(0 12 * * ? *", "cron"), None);
    }

    #[test]
    fn reads_rates_and_one_time_schedules() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 12, 20, 9, 0, 0).unwrap();
        let five = report("rate(5 minutes)", now).unwrap();
        assert!(five.contains("Description: Every 5 minutes\nInterval: 300 seconds\nRuns per day: 288"), "{five}");
        assert!(report("rate(1 hour)", now).unwrap().contains("Description: Every hour"));
        assert!(report("rate(7 minutes)", now).unwrap().contains("Runs per day: about 206"));
        assert!(!report("rate(2 days)", now).unwrap().contains("Runs per day"));
        assert_eq!(report("rate(1 hours)", now).unwrap_err(), "EventBridge expects rate(1 hour)");
        assert_eq!(report("rate(5 minute)", now).unwrap_err(), "EventBridge expects rate(5 minutes)");
        assert!(report("rate(0 minutes)", now).is_err());
        assert!(report("rate(5 weeks)", now).is_err());
        // Too large to count in seconds, but no crash.
        assert_eq!(report("rate(1000000000000000 days)", now).unwrap_err(), "That rate is too large");
        let at = report("at(2026-12-25T09:00:00)", now).unwrap();
        assert!(at.contains("Runs once: Fri 25 Dec 2026, 09:00:00") && at.contains("Relative (if UTC): in 5 days"), "{at}");
        assert!(report("at(tomorrow)", now).is_err());
    }
}
