//! Date Calculator: the time between two dates, a date plus or minus a
//! duration (optionally in business days), and one moment in several time zones.
use chrono::{DateTime, Datelike, Days, Local, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike, Utc, Weekday};
use eframe::egui;
use super::unix_time::{self, Zone};
use super::{Category, Tool};
use crate::ui::{self, theme};

/// Shown when a calculation leaves the range of dates `chrono` supports.
const OUT_OF_RANGE: &str = "The result is out of the supported range of dates";
/// Shown when a date cannot be read.
const INVALID: &str = "Enter a date like 2026-12-25 or 2026-12-25 14:30, \"today\", \"now\" or a Unix timestamp";
/// How dates are shown in the reports when their time is midnight.
const DATE_FORMAT: &str = "%a %-d %b %Y";
/// How times are shown in the time-zone list (no seconds).
const ZONE_FORMAT: &str = "%a %-d %b %Y, %H:%M";

/// Duration units, in the order they are applied (calendar units first),
/// with the largest amount each field accepts.
const UNITS: [(&str, u32); 6] = [("Years", 1_000), ("Months", 12_000), ("Weeks", 52_000), ("Days", 365_000), ("Hours", 1_000_000), ("Minutes", 10_000_000)];
/// Index of "Days" in [`UNITS`]: the amount that can count business days.
const DAYS: usize = 3;

/// What the tool calculates.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Mode {
    #[default]
    Difference,
    Add,
    Zones,
}

/// A date typed as text, with a calendar button that fills it in.
struct DateField {
    text: String,
    /// The calendar's selection, kept on the typed date once it can be read.
    date: NaiveDate,
}

impl DateField {
    /// A field holding `text`, its calendar on today until the text is read.
    fn new(text: &str) -> Self { Self { text: text.to_owned(), date: Local::now().date_naive() } }
}

/// A date as typed: a wall-clock time, or an exact instant (a timestamp or a
/// date with an offset, such as `2026-12-25T10:00:00Z`).
#[derive(Debug, PartialEq)]
enum When {
    Wall(NaiveDateTime),
    Instant(DateTime<Utc>),
}

/// Reads `today`, `tomorrow`, `yesterday`, `now` (relative to `now`),
/// `YYYY-MM-DD[ HH:MM[:SS]]`, or what the Unix Time Converter reads
/// (timestamps, ISO 8601 and RFC 2822 dates with an offset).
fn parse(text: &str, now: NaiveDateTime) -> Result<When, String> {
    let t = text.trim();
    let today = now.date().and_time(NaiveTime::MIN);
    match t.to_ascii_lowercase().as_str() {
        "now" => return Ok(When::Wall(now)),
        "today" => return Ok(When::Wall(today)),
        "tomorrow" => return Ok(When::Wall(today + TimeDelta::days(1))),
        "yesterday" => return Ok(When::Wall(today - TimeDelta::days(1))),
        _ => {}
    }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M"] {
        if let Ok(wall) = NaiveDateTime::parse_from_str(t, format) { return Ok(When::Wall(wall)); }
    }
    if let Ok(date) = NaiveDate::parse_from_str(t, "%Y-%m-%d") { return Ok(When::Wall(date.and_time(NaiveTime::MIN))); }
    unix_time::parse_instant(t, Zone::Local).map(When::Instant).map_err(|_| INVALID.to_owned())
}

/// `text` with its date replaced by `date`, keeping a time typed after it.
fn with_date(text: &str, date: NaiveDate) -> String {
    let day = date.format("%Y-%m-%d");
    match text.trim().split_once([' ', 'T']) {
        Some((_, time)) if ["%H:%M", "%H:%M:%S"].iter().any(|f| NaiveTime::parse_from_str(time, f).is_ok()) => format!("{day} {time}"),
        _ => day.to_string(),
    }
}

/// `n unit` or `n units`.
fn count(n: i64, unit: &str) -> String { format!("{n} {unit}{}", if n == 1 { "" } else { "s" }) }

/// The date, with its time unless it is midnight.
fn show(date: NaiveDateTime) -> String {
    let format = if date.time() == NaiveTime::MIN { DATE_FORMAT } else { unix_time::DATE_TIME_FORMAT };
    date.format(format).to_string()
}

/// The most whole months from `start` that do not pass `end` (`start <= end`),
/// and `start` plus those months. Adding a month keeps the day of the month,
/// or the month's last day when it is shorter (31 Jan + 1 month = 28 Feb).
fn whole_months(start: NaiveDateTime, end: NaiveDateTime) -> (u32, NaiveDateTime) {
    let estimate = (end.year() - start.year()) * 12 + end.month() as i32 - start.month() as i32;
    let mut months = u32::try_from(estimate).unwrap_or(0);
    loop {
        match start.checked_add_months(Months::new(months)) {
            Some(anchor) if anchor <= end => return (months, anchor),
            _ if months == 0 => return (0, start),
            _ => months -= 1,
        }
    }
}

/// Monday to Friday from `start` (included) to `end` (excluded).
fn business_days(start: NaiveDate, end: NaiveDate) -> i64 {
    let days = (end - start).num_days();
    let mut total = days / 7 * 5;
    let mut date = start + Days::new((days / 7 * 7) as u64);
    while date < end {
        if date.weekday().num_days_from_monday() < 5 { total += 1; }
        let Some(next) = date.succ_opt() else { break };
        date = next;
    }
    total
}

/// The report of the time from `from` to `to`.
fn difference(from: NaiveDateTime, to: NaiveDateTime) -> String {
    let backwards = to < from;
    let (start, end) = if backwards { (to, from) } else { (from, to) };
    let sign = if backwards { "-" } else { "" };
    let (months, anchor) = whole_months(start, end);
    let rest = end - anchor;
    let parts: Vec<String> = [
        (i64::from(months / 12), "year"), (i64::from(months % 12), "month"), (rest.num_days(), "day"),
        (rest.num_hours() % 24, "hour"), (rest.num_minutes() % 60, "minute"), (rest.num_seconds() % 60, "second"),
    ].into_iter().filter(|(n, _)| *n > 0).map(|(n, unit)| count(n, unit)).collect();
    let calendar = if parts.is_empty() { "none (same moment)".to_owned() } else { parts.join(", ") + if backwards { " earlier" } else { "" } };
    let total = end - start;
    let (weeks, days) = (total.num_days() / 7, total.num_days() % 7);
    let weeks = if days == 0 { count(weeks, "week") } else { format!("{}, {}", count(weeks, "week"), count(days, "day")) };
    [
        format!("From: {}", show(from)),
        format!("To: {}", show(to)),
        String::new(),
        format!("Difference: {calendar}"),
        format!("Days: {sign}{}", total.num_days()),
        format!("Weeks: {sign}{weeks}"),
        format!("Business days: {sign}{}", business_days(start.date(), end.date())),
        format!("Hours: {sign}{}", total.num_hours()),
        format!("Minutes: {sign}{}", total.num_minutes()),
        format!("Seconds: {sign}{}", total.num_seconds()),
    ].join("\n")
}

/// `date` moved by `n` business days (Monday to Friday), forward or back.
/// From a weekend, counting starts at the nearest weekday behind, so one
/// business day after a Saturday is the Monday.
fn shift_business_days(date: NaiveDate, n: u32, back: bool) -> Option<NaiveDate> {
    if n == 0 { return Some(date); }
    let step = |d: NaiveDate| if back { d.pred_opt() } else { d.succ_opt() };
    let weekend = |d: NaiveDate| matches!(d.weekday(), Weekday::Sat | Weekday::Sun);
    let mut date = date;
    while weekend(date) { date = if back { date.succ_opt()? } else { date.pred_opt()? }; }
    // Five business days are always one calendar week.
    let weeks = Days::new(u64::from(n / 5) * 7);
    date = if back { date.checked_sub_days(weeks)? } else { date.checked_add_days(weeks)? };
    for _ in 0..n % 5 {
        date = step(date)?;
        while weekend(date) { date = step(date)?; }
    }
    Some(date)
}

/// `start` plus (or minus) `amounts`, in the order of [`UNITS`]. With
/// `business`, the days are business days.
fn shift(start: NaiveDateTime, amounts: [u32; 6], back: bool, business: bool) -> Option<NaiveDateTime> {
    let [years, months, weeks, days, hours, minutes] = amounts.map(i64::from);
    let months = Months::new(u32::try_from(years * 12 + months).ok()?);
    let mut date = if back { start.checked_sub_months(months)? } else { start.checked_add_months(months)? };
    let mut delta = TimeDelta::try_weeks(weeks)? + TimeDelta::try_hours(hours)? + TimeDelta::try_minutes(minutes)?;
    if business {
        date = shift_business_days(date.date(), amounts[DAYS], back)?.and_time(date.time());
    } else {
        delta += TimeDelta::try_days(days)?;
    }
    if back { date.checked_sub_signed(delta) } else { date.checked_add_signed(delta) }
}

/// The report of a shifted date; `now` gives the relative time.
fn shifted(start: NaiveDateTime, result: NaiveDateTime, now: NaiveDateTime) -> String {
    let iso = if result.time() == NaiveTime::MIN { result.format("%Y-%m-%d").to_string() } else { result.format("%Y-%m-%dT%H:%M:%S").to_string() };
    [
        format!("Start: {}", show(start)),
        format!("Result: {}", show(result)),
        String::new(),
        format!("ISO 8601: {iso}"),
        format!("Day of week: {}", result.format("%A")),
        format!("Day of year: {}", result.ordinal()),
        format!("ISO week: {}", result.iso_week().week()),
        format!("Relative: {}", unix_time::relative(result - now)),
    ].join("\n")
}

/// `instant` in `source` (first), local time, UTC and common zones, each with
/// its UTC offset and a note when the date differs from the source's.
fn zones(instant: DateTime<Utc>, source: Zone) -> String {
    let source_date = source.view(instant).0.date_naive();
    let mut list = vec![source];
    for zone in [Zone::Local, Zone::Utc].into_iter().chain(unix_time::COMMON_ZONES.map(Zone::Named)) {
        if !list.contains(&zone) { list.push(zone); }
    }
    let mut lines = Vec::new();
    for (i, zone) in list.into_iter().enumerate() {
        let (time, abbreviation) = zone.view(instant);
        let offset = format!("UTC{}", time.format("%:z"));
        // Zones without a known abbreviation show their offset instead ("+04").
        let name = if abbreviation == "UTC" { abbreviation } else if abbreviation.starts_with(['+', '-']) { offset } else { format!("{abbreviation}, {offset}") };
        let day = match (time.date_naive() - source_date).num_days() {
            0 => String::new(),
            1 => " · next day".to_owned(),
            -1 => " · previous day".to_owned(),
            n => format!(" · {n:+} days"),
        };
        lines.push(format!("{}: {} ({name}){day}", zone.label(), time.format(ZONE_FORMAT)));
        if i == 0 { lines.push(String::new()); }
    }
    lines.join("\n")
}

/// The Date Calculator tool.
pub struct DateCalculator {
    mode: Mode,
    /// Difference: the two dates.
    from: DateField,
    to: DateField,
    /// Add / Subtract: the start date and the amounts of [`UNITS`].
    start: DateField,
    amounts: [u32; 6],
    subtract: bool,
    /// Count the days as business days (Monday to Friday).
    business_days: bool,
    /// Time zones: the moment, as a wall-clock time in `zone`.
    moment: DateField,
    zone: Zone,
    /// Search text typed in the time-zone menu.
    zone_filter: String,
    /// The report of the selected mode.
    output: String,
    error: String,
}

impl Default for DateCalculator {
    fn default() -> Self {
        Self {
            mode: Mode::Difference, from: DateField::new("today"), to: DateField::new(""),
            start: DateField::new("today"), amounts: [0, 0, 0, 30, 0, 0], subtract: false, business_days: false,
            moment: DateField::new("now"), zone: Zone::Local, zone_filter: String::new(),
            output: String::new(), error: String::new(),
        }
    }
}

/// A date field with a calendar button that fills it in (keeping a typed time).
fn date_input(ui: &mut egui::Ui, id: &str, field: &mut DateField, hint: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut field.text).font(egui::TextStyle::Monospace).desired_width(230.0).hint_text(hint));
        let calendar = egui_extras::DatePickerButton::new(&mut field.date).id_salt(id).format("").calendar_week(true);
        if ui.add(calendar).on_hover_text("Pick a date").changed() { field.text = with_date(&field.text, field.date); }
    });
}

/// A muted label for the first column of the form.
fn label(ui: &mut egui::Ui, text: &str) { ui.label(egui::RichText::new(text).color(theme::MUTED)); }

impl DateCalculator {
    /// The report of the selected mode; empty while a required date is missing.
    /// Keeps each calendar on the date its field holds.
    fn calculate(&mut self) -> Result<String, String> {
        let now = Local::now().naive_local().with_nanosecond(0).unwrap_or_default();
        // Wall-clock dates are taken as they are; instants as the local time they show.
        let read = |field: &mut DateField| -> Result<NaiveDateTime, String> {
            let wall = match parse(&field.text, now)? { When::Wall(wall) => wall, When::Instant(instant) => instant.with_timezone(&Local).naive_local() };
            field.date = wall.date();
            Ok(wall)
        };
        match self.mode {
            Mode::Difference => {
                if self.from.text.trim().is_empty() || self.to.text.trim().is_empty() { return Ok(String::new()); }
                Ok(difference(read(&mut self.from)?, read(&mut self.to)?))
            }
            Mode::Add => {
                if self.start.text.trim().is_empty() { return Ok(String::new()); }
                let start = read(&mut self.start)?;
                let result = shift(start, self.amounts, self.subtract, self.business_days).ok_or(OUT_OF_RANGE)?;
                Ok(shifted(start, result, now))
            }
            Mode::Zones => {
                if self.moment.text.trim().is_empty() { return Ok(String::new()); }
                // "now" and "today" are read in the chosen zone.
                let now_there = self.zone.view(Utc::now()).0.naive_local().with_nanosecond(0).unwrap_or_default();
                let instant = match parse(&self.moment.text, now_there)? {
                    When::Wall(wall) => self.zone.instant(wall).ok_or(unix_time::DST_GAP)?,
                    When::Instant(instant) => instant,
                };
                self.moment.date = self.zone.view(instant).0.date_naive();
                Ok(zones(instant, self.zone))
            }
        }
    }

    /// Fills the selected mode with an example.
    fn sample(&mut self) {
        match self.mode {
            Mode::Difference => { self.from.text = "2026-01-01".into(); self.to.text = "2026-12-25 17:30".into(); }
            Mode::Add => { self.start.text = "2026-11-19".into(); self.amounts = [0, 1, 0, 10, 0, 0]; self.subtract = false; self.business_days = true; }
            Mode::Zones => self.moment.text = "2026-11-19 09:00".into(),
        }
    }

    /// Empties the selected mode.
    fn clear(&mut self) {
        match self.mode {
            Mode::Difference => { self.from.text.clear(); self.to.text.clear(); }
            Mode::Add => { self.start.text.clear(); self.amounts = [0; 6]; }
            Mode::Zones => self.moment.text.clear(),
        }
    }

    /// The fields of the selected mode.
    fn form(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("date-calc-form").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| match self.mode {
            Mode::Difference => {
                label(ui, "From");
                date_input(ui, "date-calc-from", &mut self.from, "today, 2026-01-01…");
                ui.end_row();
                label(ui, "To");
                date_input(ui, "date-calc-to", &mut self.to, "2026-12-25 17:30…");
                ui.end_row();
            }
            Mode::Add => {
                label(ui, "Start");
                date_input(ui, "date-calc-start", &mut self.start, "today, 2026-01-01…");
                ui.end_row();
                label(ui, "Operation");
                ui::segmented(ui, &mut self.subtract, &[(false, "+  Add"), (true, "\u{2212}  Subtract")]);
                ui.end_row();
                for (i, (unit, max)) in UNITS.iter().enumerate() {
                    label(ui, unit);
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.amounts[i]).range(0..=*max));
                        if i == DAYS { ui.checkbox(&mut self.business_days, "Business days (Mon\u{2013}Fri)"); }
                    });
                    ui.end_row();
                }
            }
            Mode::Zones => {
                label(ui, "Date and time");
                date_input(ui, "date-calc-moment", &mut self.moment, "now, 2026-11-19 09:00…");
                ui.end_row();
                label(ui, "Time zone");
                unix_time::zone_picker(ui, "date-calc-zone", &mut self.zone, &mut self.zone_filter);
                ui.end_row();
            }
        });
    }
}

impl Tool for DateCalculator {
    fn name(&self) -> &'static str { "Date Calculator" }
    fn icon(&self) -> &'static str { ui::icon::CALENDAR_PLUS }
    fn category(&self) -> Category { Category::Date }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Count the time between two dates, add or subtract a duration, or see a moment in several time zones.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            ui::toolbar(left, "Calculate:", |ui| {
                if ui::sample_button(ui) { self.sample(); }
                if ui::clear_button(ui) { self.clear(); }
                ui.separator();
                ui::segmented(ui, &mut self.mode, &[(Mode::Difference, "Difference"), (Mode::Add, "Add / Subtract"), (Mode::Zones, "Time zones")]);
            });
            left.add_space(4.0);
            self.form(left);
            // Cheap enough to redo every frame, which keeps "now" and the calendars current.
            match self.calculate() {
                Ok(report) => { self.output = report; self.error.clear(); }
                Err(e) => { self.output.clear(); self.error = e; }
            }
            ui::toolbar(right, "Result:", |ui| ui::copy_button(ui, &self.output));
            ui::error_label(right, &self.error);
            ui::properties(right, "result", &self.output);
        });
    }

    fn load_sample(&mut self) {
        self.mode = Mode::Difference;
        self.sample();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap_or_else(|_| NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap().and_time(NaiveTime::MIN))
    }

    #[test]
    fn reads_dates_and_keywords() {
        let now = at("2026-11-19 14:30");
        assert_eq!(parse("today", now), Ok(When::Wall(at("2026-11-19"))));
        assert_eq!(parse(" Tomorrow ", now), Ok(When::Wall(at("2026-11-20"))));
        assert_eq!(parse("2026-12-25T08:15", now), Ok(When::Wall(at("2026-12-25 08:15"))));
        assert_eq!(parse("2026-12-25T08:15:00Z", now), Ok(When::Instant(DateTime::parse_from_rfc3339("2026-12-25T08:15:00Z").unwrap().into())));
        assert_eq!(parse("next week", now), Err(INVALID.to_owned()));
        assert_eq!(with_date("2026-01-01 09:30", NaiveDate::from_ymd_opt(2026, 2, 3).unwrap()), "2026-02-03 09:30");
        assert_eq!(with_date("today", NaiveDate::from_ymd_opt(2026, 2, 3).unwrap()), "2026-02-03");
    }

    #[test]
    fn counts_the_time_between_dates() {
        let out = difference(at("2026-01-31"), at("2026-03-01 06:00"));
        // 31 Jan + 1 month = 28 Feb, then 1 day and 6 hours to 1 Mar 06:00.
        assert!(out.contains("Difference: 1 month, 1 day, 6 hours\n"), "{out}");
        assert!(out.contains("Days: 29\nWeeks: 4 weeks, 1 day\n"), "{out}");
        assert!(out.contains("Hours: 702\n"), "{out}");
        let back = difference(at("2026-12-25"), at("2026-12-18"));
        assert!(back.contains("Difference: 7 days earlier\nDays: -7\nWeeks: -1 week\nBusiness days: -5"), "{back}");
        assert!(difference(at("2026-05-01"), at("2026-05-01")).contains("Difference: none (same moment)"));
    }

    #[test]
    fn handles_leap_days_and_timestamps() {
        // 29 Feb + 1 year is 28 Feb: exactly one year.
        assert!(difference(at("2024-02-29"), at("2025-02-28")).contains("Difference: 1 year\n"));
        assert!(difference(at("2024-02-28"), at("2024-03-01")).contains("Days: 2\n"));
        let monday = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        assert_eq!(business_days(monday, monday), 0);
        let now = at("2026-11-19 14:30");
        assert!(matches!(parse("1700000000", now), Ok(When::Instant(_))));
        assert_eq!(with_date("2026-01-01T08:00", NaiveDate::from_ymd_opt(2026, 3, 4).unwrap()), "2026-03-04 08:00");
    }

    #[test]
    fn counts_business_days() {
        let date = |d: u32| NaiveDate::from_ymd_opt(2026, 11, d).unwrap();
        // Mon 16 Nov to Mon 23 Nov 2026: one week.
        assert_eq!(business_days(date(16), date(23)), 5);
        // Fri 20 to Mon 23: only the Friday.
        assert_eq!(business_days(date(20), date(23)), 1);
        assert_eq!(business_days(date(21), date(23)), 0);
        // Fri + 1 business day = Mon; Sat + 1 = Mon; Mon - 1 = Fri; Sun - 1 = Fri.
        assert_eq!(shift_business_days(date(20), 1, false), Some(date(23)));
        assert_eq!(shift_business_days(date(21), 1, false), Some(date(23)));
        assert_eq!(shift_business_days(date(23), 1, true), Some(date(20)));
        assert_eq!(shift_business_days(date(22), 1, true), Some(date(20)));
        // Thu 19 + 7 business days = Mon 30.
        assert_eq!(shift_business_days(date(19), 7, false), Some(date(30)));
        assert_eq!(shift_business_days(date(21), 0, false), Some(date(21)));
    }

    #[test]
    fn adds_and_subtracts_durations() {
        let start = at("2026-01-31 09:00");
        assert_eq!(shift(start, [0, 1, 0, 0, 0, 0], false, false), Some(at("2026-02-28 09:00")));
        assert_eq!(shift(start, [1, 0, 1, 2, 3, 30], false, false), Some(at("2027-02-09 12:30")));
        assert_eq!(shift(start, [0, 0, 0, 10, 0, 0], true, false), Some(at("2026-01-21 09:00")));
        // Sat 31 Jan 2026 + 1 business day = Mon 2 Feb, keeping the time.
        assert_eq!(shift(start, [0, 0, 0, 1, 0, 0], false, true), Some(at("2026-02-02 09:00")));
        assert_eq!(shift(start, [1_000, 0, 0, 0, 0, 0], false, false), Some(at("3026-01-31 09:00")));
        let report = shifted(start, at("2026-12-25"), at("2026-12-20"));
        assert!(report.contains("Result: Fri 25 Dec 2026\n\nISO 8601: 2026-12-25\nDay of week: Friday\nDay of year: 359\nISO week: 52\nRelative: in 5 days"), "{report}");
    }

    #[test]
    fn lists_a_moment_in_other_zones() {
        let instant = DateTime::parse_from_rfc3339("2026-11-19T23:30:00-05:00").unwrap().with_timezone(&Utc);
        let out = zones(instant, Zone::Named(chrono_tz::Tz::America__Toronto));
        assert!(out.starts_with("America/Toronto: Thu 19 Nov 2026, 23:30 (EST, UTC-05:00)\n\n"), "{out}");
        assert!(out.contains("UTC: Fri 20 Nov 2026, 04:30 (UTC) · next day"), "{out}");
        assert!(out.contains("Asia/Kolkata: Fri 20 Nov 2026, 10:00 (IST, UTC+05:30) · next day"), "{out}");
        assert!(out.contains("America/Los Angeles: Thu 19 Nov 2026, 20:30 (PST, UTC-08:00)\n"), "{out}");
        // Toronto is listed once.
        assert_eq!(out.matches("America/Toronto").count(), 1, "{out}");
    }
}
