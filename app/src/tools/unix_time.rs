//! Unix Time Converter: timestamps ↔ dates, with a calendar, time zones and
//! relative time ("3 days ago").
use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat, TimeDelta, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon, theme, Highlight};

const SAMPLE: &str = "1700000000";

/// Time zones offered before searching the full IANA list.
const COMMON_ZONES: [Tz; 16] = [
    Tz::America__Toronto, Tz::America__New_York, Tz::America__Chicago, Tz::America__Denver, Tz::America__Los_Angeles,
    Tz::America__Sao_Paulo, Tz::Europe__London, Tz::Europe__Paris, Tz::Europe__Berlin, Tz::Africa__Johannesburg,
    Tz::Asia__Dubai, Tz::Asia__Kolkata, Tz::Asia__Singapore, Tz::Asia__Tokyo, Tz::Australia__Sydney, Tz::Pacific__Auckland,
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Zone { Local, Utc, Named(Tz) }

impl Zone {
    fn label(self) -> String {
        match self { Zone::Local => "Local time".into(), Zone::Utc => "UTC".into(), Zone::Named(tz) => tz.name().replace('_', " ") }
    }

    /// `instant` as seen in this zone, plus the zone's abbreviation (e.g. "EST").
    fn view(self, instant: DateTime<Utc>) -> (DateTime<FixedOffset>, String) {
        match self {
            Zone::Local => { let t = instant.with_timezone(&Local); (t.fixed_offset(), t.format("%:z").to_string()) }
            Zone::Utc => (instant.fixed_offset(), "UTC".into()),
            Zone::Named(tz) => { let t = instant.with_timezone(&tz); (t.fixed_offset(), t.format("%Z").to_string()) }
        }
    }

    /// The instant at a wall-clock time in this zone (`None` in a daylight-saving gap).
    fn instant(self, naive: NaiveDateTime) -> Option<DateTime<Utc>> {
        match self {
            Zone::Local => Local.from_local_datetime(&naive).earliest().map(|t| t.with_timezone(&Utc)),
            Zone::Utc => Some(Utc.from_utc_datetime(&naive)),
            Zone::Named(tz) => tz.from_local_datetime(&naive).earliest().map(|t| t.with_timezone(&Utc)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Unit { Auto, Seconds, Millis, Micros, Nanos }

impl Unit {
    const ALL: [Unit; 5] = [Unit::Auto, Unit::Seconds, Unit::Millis, Unit::Micros, Unit::Nanos];

    fn label(self) -> &'static str {
        match self { Unit::Auto => "Auto", Unit::Seconds => "Seconds", Unit::Millis => "Milliseconds", Unit::Micros => "Microseconds", Unit::Nanos => "Nanoseconds" }
    }

    fn nanos(self) -> i128 {
        match self { Unit::Auto | Unit::Seconds => 1_000_000_000, Unit::Millis => 1_000_000, Unit::Micros => 1_000, Unit::Nanos => 1 }
    }

    /// Guesses the unit from the number of digits: seconds cover years 1973–5138
    /// with 9–11 digits, each finer unit adds 3 digits.
    fn detect(n: i128) -> Unit {
        match n.unsigned_abs() {
            a if a < 100_000_000_000 => Unit::Seconds,
            a if a < 100_000_000_000_000 => Unit::Millis,
            a if a < 100_000_000_000_000_000 => Unit::Micros,
            _ => Unit::Nanos,
        }
    }
}

/// Reads a timestamp (integer or decimal, in `unit`), an ISO 8601 / RFC 2822
/// date, a `YYYY-MM-DD[ HH:MM[:SS]]` wall-clock time in `zone`, or "now".
/// Returns the instant and, for numbers, the unit that was used.
fn parse_input(text: &str, unit: Unit, zone: Zone) -> Result<(DateTime<Utc>, Option<Unit>), String> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("now") { return Ok((Utc::now(), None)); }
    let compact = t.replace('_', "");
    if let Ok(n) = compact.parse::<i128>() {
        let unit = if unit == Unit::Auto { Unit::detect(n) } else { unit };
        return from_nanos(n.checked_mul(unit.nanos()).ok_or("Timestamp out of range")?).map(|d| (d, Some(unit)));
    }
    if let Ok(f) = compact.parse::<f64>() {
        let unit = if unit == Unit::Auto { Unit::detect(f as i128) } else { unit };
        return from_nanos((f * unit.nanos() as f64).round() as i128).map(|d| (d, Some(unit)));
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(t) { return Ok((d.with_timezone(&Utc), None)); }
    if let Ok(d) = DateTime::parse_from_rfc2822(t) { return Ok((d.with_timezone(&Utc), None)); }
    for format in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(t, format) {
            return zone.instant(naive).map(|d| (d, None)).ok_or_else(|| "That time does not exist in this zone (daylight-saving gap)".into());
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(t, "%Y-%m-%d") {
        return zone.instant(date.and_time(NaiveTime::MIN)).map(|d| (d, None)).ok_or_else(|| "That time does not exist in this zone".into());
    }
    Err("Enter a Unix timestamp (s, ms, µs or ns), a date like 2024-05-17 14:30 or 2024-05-17T14:30:00Z, or \"now\"".into())
}

fn from_nanos(nanos: i128) -> Result<DateTime<Utc>, String> {
    let secs = i64::try_from(nanos.div_euclid(1_000_000_000)).map_err(|_| "Timestamp out of range")?;
    DateTime::from_timestamp(secs, nanos.rem_euclid(1_000_000_000) as u32).ok_or_else(|| "Timestamp out of range".into())
}

/// "in 2 hours", "3 days ago", "just now".
pub(super) fn relative(delta: TimeDelta) -> String {
    let seconds = delta.num_seconds();
    let abs = seconds.unsigned_abs();
    if abs < 10 { return "just now".into(); }
    let (n, unit) = match abs {
        a if a < 60 => (a, "second"),
        a if a < 3_600 => (a / 60, "minute"),
        a if a < 86_400 => (a / 3_600, "hour"),
        a if a < 30 * 86_400 => (a / 86_400, "day"),
        a if a < 365 * 86_400 => (a / (30 * 86_400), "month"),
        a => (a / (365 * 86_400), "year"),
    };
    let plural = if n == 1 { "" } else { "s" };
    if seconds > 0 { format!("in {n} {unit}{plural}") } else { format!("{n} {unit}{plural} ago") }
}

/// The "Key: value" report shown on the right.
fn report(instant: DateTime<Utc>, zone: Zone, detected: Option<Unit>, now: DateTime<Utc>) -> String {
    let (zoned, abbreviation) = zone.view(instant);
    let mut lines = Vec::new();
    if let Some(unit) = detected { lines.push(format!("Read as: {}", unit.label().to_lowercase())); }
    lines.push(format!("Unix seconds: {}", instant.timestamp()));
    lines.push(format!("Unix milliseconds: {}", instant.timestamp_millis()));
    lines.push(format!("Unix microseconds: {}", instant.timestamp_micros()));
    lines.push(String::new());
    lines.push(format!("UTC: {}", instant.format("%a %-d %b %Y, %H:%M:%S")));
    if zone != Zone::Utc { lines.push(format!("{}: {} ({abbreviation})", zone.label(), zoned.format("%a %-d %b %Y, %H:%M:%S"))); }
    lines.push(format!("Relative: {}", relative(instant - now)));
    lines.push(String::new());
    lines.push(format!("ISO 8601: {}", instant.to_rfc3339_opts(SecondsFormat::AutoSi, true)));
    if zone != Zone::Utc { lines.push(format!("ISO 8601 ({}): {}", zone.label(), zoned.to_rfc3339_opts(SecondsFormat::AutoSi, false))); }
    lines.push(format!("RFC 2822: {}", zoned.to_rfc2822()));
    lines.push(format!("Day of year: {} · ISO week {}", zoned.ordinal(), zoned.iso_week().week()));
    lines.join("\n")
}

pub struct UnixTime {
    input: String,
    unit: Unit,
    zone: Zone,
    zone_filter: String,
    /// Calendar and clock fields, kept in sync with the input (in `zone`).
    date: NaiveDate,
    time: (u32, u32, u32),
    output: String,
    error: String,
}

impl Default for UnixTime {
    fn default() -> Self {
        let now = Local::now().naive_local();
        Self {
            input: String::new(), unit: Unit::Auto, zone: Zone::Local, zone_filter: String::new(),
            date: now.date(), time: (now.hour(), now.minute(), now.second()), output: String::new(), error: String::new(),
        }
    }
}

impl UnixTime {
    /// Recomputes the report from the input. With `sync_picker`, the calendar
    /// and clock fields follow the parsed instant.
    fn update(&mut self, sync_picker: bool) {
        if self.input.trim().is_empty() { self.output.clear(); self.error.clear(); return; }
        match parse_input(&self.input, self.unit, self.zone) {
            Ok((instant, detected)) => {
                self.output = report(instant, self.zone, detected, Utc::now());
                self.error.clear();
                if sync_picker {
                    let local = self.zone.view(instant).0.naive_local();
                    self.date = local.date();
                    self.time = (local.hour(), local.minute(), local.second());
                }
            }
            Err(e) => { self.output.clear(); self.error = e; }
        }
    }

    /// Writes the calendar/clock value into the input, as a timestamp in the chosen unit.
    fn apply_picker(&mut self) {
        let (h, m, s) = self.time;
        let Some(naive) = NaiveTime::from_hms_opt(h, m, s).map(|t| self.date.and_time(t)) else { return };
        match self.zone.instant(naive) {
            Some(instant) => {
                self.input = match self.unit {
                    Unit::Millis => instant.timestamp_millis().to_string(),
                    Unit::Micros => instant.timestamp_micros().to_string(),
                    Unit::Nanos => instant.timestamp_nanos_opt().map_or_else(|| instant.timestamp().to_string(), |n| n.to_string()),
                    Unit::Auto | Unit::Seconds => instant.timestamp().to_string(),
                };
                self.update(false);
            }
            None => self.error = "That time does not exist in this zone (daylight-saving gap)".into(),
        }
    }

    /// The time-zone menu: local time, UTC and common zones; typing searches all IANA zones.
    fn zone_picker(&mut self, ui: &mut egui::Ui) -> bool {
        let before = self.zone;
        egui::ComboBox::from_id_salt("unix-zone").width(210.0).height(320.0).selected_text(self.zone.label()).show_ui(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.zone_filter).hint_text(format!("{}  Search all zones…", icon::MAGNIFYING_GLASS)));
            ui.separator();
            let query = self.zone_filter.to_lowercase().replace(' ', "_");
            if query.is_empty() {
                ui.selectable_value(&mut self.zone, Zone::Local, Zone::Local.label());
                ui.selectable_value(&mut self.zone, Zone::Utc, "UTC");
                ui.separator();
                for tz in COMMON_ZONES { ui.selectable_value(&mut self.zone, Zone::Named(tz), Zone::Named(tz).label()); }
            } else {
                for tz in chrono_tz::TZ_VARIANTS.iter().filter(|tz| tz.name().to_lowercase().contains(&query)) {
                    ui.selectable_value(&mut self.zone, Zone::Named(*tz), Zone::Named(*tz).label());
                }
            }
        });
        self.zone != before
    }
}

impl Tool for UnixTime {
    fn name(&self) -> &'static str { "Unix Time Converter" }
    fn icon(&self) -> &'static str { icon::CLOCK }
    fn category(&self) -> Category { Category::Date }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // Keep "Now" and the relative time current.
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        ui::description(ui, "Type a Unix timestamp (s, ms, µs or ns are detected), a date (ISO 8601, RFC 2822) or \"now\", or pick a date.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "Timestamp or date:", |ui| {
                changed |= ui::paste_button(ui, &mut self.input, &mut self.error);
                if ui::sample_button(ui) { self.load_sample(); }
                if ui::clear_button(ui) { self.input.clear(); changed = true; }
                ui.separator();
                if ui.add(ui::icon_button(icon::CLOCK_COUNTER_CLOCKWISE, "Now", true)).clicked() {
                    self.input = Utc::now().timestamp().to_string();
                    changed = true;
                }
            });
            changed |= ui::editor_frame().show(left, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.input).frame(false).font(egui::FontId::monospace(16.0))
                    .desired_width(f32::INFINITY).hint_text("1700000000"))
            }).inner.changed();
            left.add_space(12.0);

            let mut options_changed = false;
            let mut picker_changed = false;
            egui::Grid::new("unix-options").num_columns(2).spacing([12.0, 10.0]).show(left, |ui| {
                ui.label(egui::RichText::new("Unit").color(theme::MUTED));
                egui::ComboBox::from_id_salt("unix-unit").width(210.0).selected_text(self.unit.label()).show_ui(ui, |ui| {
                    for unit in Unit::ALL { options_changed |= ui.selectable_value(&mut self.unit, unit, unit.label()).changed(); }
                });
                ui.end_row();

                ui.label(egui::RichText::new("Time zone").color(theme::MUTED));
                options_changed |= self.zone_picker(ui);
                ui.end_row();

                ui.label(egui::RichText::new("Date").color(theme::MUTED));
                picker_changed |= ui.add(egui_extras::DatePickerButton::new(&mut self.date).id_salt("unix-date").calendar_week(true)).changed();
                ui.end_row();

                ui.label(egui::RichText::new("Time").color(theme::MUTED));
                ui.horizontal(|ui| {
                    let (h, m, s) = &mut self.time;
                    picker_changed |= ui.add(egui::DragValue::new(h).range(0..=23).custom_formatter(|v, _| format!("{v:02}"))).changed();
                    ui.label(":");
                    picker_changed |= ui.add(egui::DragValue::new(m).range(0..=59).custom_formatter(|v, _| format!("{v:02}"))).changed();
                    ui.label(":");
                    picker_changed |= ui.add(egui::DragValue::new(s).range(0..=59).custom_formatter(|v, _| format!("{v:02}"))).changed();
                });
                ui.end_row();
            });
            left.add_space(12.0);
            left.label(egui::RichText::new(format!("{}  Now: {}", icon::CLOCK, Utc::now().timestamp())).monospace().color(theme::SUBTLE));

            if picker_changed { self.apply_picker(); }
            else if changed || options_changed { self.update(true); }
            else if !self.input.trim().is_empty() && self.error.is_empty() { self.update(false); } // keeps "Relative" current

            let reuse = ui::toolbar(right, "Result:", |ui| {
                ui::copy_button(ui, &self.output);
                super::reuse_menu(ui, &self.output, "Unix Time Converter")
            });
            if reuse { self.input = self.output.lines().find_map(|l| l.strip_prefix("Unix seconds: ")).unwrap_or_default().to_owned(); self.update(true); }
            ui::error_label(right, &self.error);
            ui::code_editor(right, "result", &mut self.output, "", Highlight::KeyValue);
        });
    }

    fn load_sample(&mut self) {
        self.input = SAMPLE.into();
        self.update(true);
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.input = text.trim().to_owned();
        self.update(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(text: &str) -> DateTime<Utc> { DateTime::parse_from_rfc3339(text).unwrap().with_timezone(&Utc) }

    #[test]
    fn detects_units() {
        let expected = utc("2023-11-14T22:13:20Z");
        for (input, unit) in [("1700000000", Unit::Seconds), ("1700000000000", Unit::Millis), ("1700000000000000", Unit::Micros), ("1700000000000000000", Unit::Nanos)] {
            assert_eq!(parse_input(input, Unit::Auto, Zone::Utc).unwrap(), (expected, Some(unit)), "{input}");
        }
        assert_eq!(parse_input("1700000000.5", Unit::Auto, Zone::Utc).unwrap().0, utc("2023-11-14T22:13:20.5Z"));
        assert_eq!(parse_input("-86400", Unit::Auto, Zone::Utc).unwrap().0, utc("1969-12-31T00:00:00Z"));
        // An explicit unit wins over detection.
        assert_eq!(parse_input("1700000000", Unit::Millis, Zone::Utc).unwrap().0, utc("1970-01-20T16:13:20Z"));
    }

    #[test]
    fn parses_dates_in_the_selected_zone() {
        assert_eq!(parse_input("2024-05-17T14:30:00Z", Unit::Auto, Zone::Utc).unwrap().0, utc("2024-05-17T14:30:00Z"));
        assert_eq!(parse_input("Fri, 17 May 2024 14:30:00 +0000", Unit::Auto, Zone::Utc).unwrap().0, utc("2024-05-17T14:30:00Z"));
        // Wall-clock times are read in the chosen zone (Toronto is UTC-4 in May).
        let toronto = Zone::Named(Tz::America__Toronto);
        assert_eq!(parse_input("2024-05-17 14:30", Unit::Auto, toronto).unwrap().0, utc("2024-05-17T18:30:00Z"));
        assert_eq!(parse_input("2024-05-17", Unit::Auto, toronto).unwrap().0, utc("2024-05-17T04:00:00Z"));
        // 02:30 on 10 March 2024 does not exist in Toronto (clocks jump forward).
        assert!(parse_input("2024-03-10 02:30", Unit::Auto, toronto).is_err());
        assert!(parse_input("yesterday", Unit::Auto, Zone::Utc).is_err());
    }

    #[test]
    fn reports_and_relative_time() {
        let instant = utc("2023-11-14T22:13:20Z");
        let out = report(instant, Zone::Named(Tz::America__Toronto), Some(Unit::Seconds), instant + TimeDelta::days(3));
        assert!(out.contains("Unix milliseconds: 1700000000000"), "{out}");
        assert!(out.contains("UTC: Tue 14 Nov 2023, 22:13:20"), "{out}");
        assert!(out.contains("America/Toronto: Tue 14 Nov 2023, 17:13:20 (EST)"), "{out}");
        assert!(out.contains("Relative: 3 days ago"), "{out}");
        assert!(out.contains("ISO 8601: 2023-11-14T22:13:20Z"), "{out}");
        assert!(out.contains("ISO 8601 (America/Toronto): 2023-11-14T17:13:20-05:00"), "{out}");
        assert_eq!(relative(TimeDelta::hours(2)), "in 2 hours");
        assert_eq!(relative(TimeDelta::seconds(-61)), "1 minute ago");
        assert_eq!(relative(TimeDelta::seconds(3)), "just now");
    }
}
