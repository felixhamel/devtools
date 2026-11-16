//! UUID v4 / v7 and ULID: bulk generation and decoding (version, variant and
//! the embedded timestamp of v7 UUIDs and ULIDs).
use chrono::{DateTime, SecondsFormat, Utc};
use eframe::egui;
use super::{text::random_bytes, unix_time::relative, Category, Tool};
use crate::ui::{self, icon, theme, TextIo};

/// ULID alphabet (Crockford's Base32: no I, L, O, U).
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { UuidV4, UuidV7, Ulid }

/// How UUIDs are written. ULIDs are always uppercase (their canonical form).
#[derive(Clone, Copy, Default, PartialEq)]
struct Format { uppercase: bool, no_hyphens: bool }

fn now_millis() -> Result<u64, String> {
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis();
    u64::try_from(millis).ok().filter(|m| *m < 1 << 48).ok_or_else(|| "The current time does not fit in 48 bits".into())
}

fn format_uuid(bytes: [u8; 16], format: Format) -> String {
    let h: String = bytes.iter().map(|b| if format.uppercase { format!("{b:02X}") } else { format!("{b:02x}") }).collect();
    if format.no_hyphens { h } else { format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..]) }
}

fn encode_ulid(bytes: [u8; 16]) -> String {
    let mut value = u128::from_be_bytes(bytes);
    let mut encoded = [b'0'; 26];
    for c in encoded.iter_mut().rev() { *c = CROCKFORD[(value & 31) as usize]; value >>= 5; }
    String::from_utf8(encoded.to_vec()).expect("Crockford alphabet is ASCII")
}

/// Builds an identifier from a timestamp (v7, ULID) and 16 random bytes.
/// v4: random with version/variant bits. v7: 48-bit Unix milliseconds, then
/// random (RFC 9562). ULID: 48-bit milliseconds + 80 random bits.
fn build(kind: Kind, millis: u64, mut bytes: [u8; 16], format: Format) -> String {
    if kind != Kind::UuidV4 { bytes[..6].copy_from_slice(&millis.to_be_bytes()[2..]); }
    match kind {
        Kind::Ulid => encode_ulid(bytes),
        Kind::UuidV4 | Kind::UuidV7 => {
            let version = if kind == Kind::UuidV4 { 0x40 } else { 0x70 };
            bytes[6] = (bytes[6] & 0x0f) | version;
            bytes[8] = (bytes[8] & 0x3f) | 0x80; // RFC variant
            format_uuid(bytes, format)
        }
    }
}

fn generate(kind: Kind, format: Format) -> Result<String, String> {
    Ok(build(kind, now_millis()?, random_bytes::<16>()?, format))
}

/// Accepts `xxxxxxxx-xxxx-…`, 32 hex digits, `{…}` and `urn:uuid:…`.
fn parse_uuid(text: &str) -> Option<[u8; 16]> {
    let s = text.trim().trim_start_matches("urn:uuid:").trim_start_matches('{').trim_end_matches('}');
    let hex: String = if s.len() == 36 && [8, 13, 18, 23].iter().all(|&i| s.as_bytes().get(i) == Some(&b'-')) { s.replace('-', "") } else { s.to_owned() };
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { return None; }
    Some(u128::from_str_radix(&hex, 16).ok()?.to_be_bytes())
}

fn parse_ulid(text: &str) -> Result<u128, String> {
    let mut value = 0u128;
    for (i, c) in text.bytes().enumerate() {
        let digit = CROCKFORD.iter().position(|&x| x == c.to_ascii_uppercase()).ok_or("Invalid ULID character")? as u128;
        if i == 0 && digit > 7 { return Err("ULID exceeds 128 bits".into()); }
        value = (value << 5) | digit;
    }
    Ok(value)
}

fn timestamp_line(millis: u64) -> String {
    match DateTime::from_timestamp_millis(millis as i64) {
        Some(t) => format!("Timestamp: {} ({})", t.to_rfc3339_opts(SecondsFormat::Millis, true), relative(t - Utc::now())),
        None => format!("Timestamp: {millis} ms (out of range)"),
    }
}

/// Describes one ULID or UUID as "Key: value" lines.
fn decode_identifier(input: &str) -> Result<String, String> {
    let s = input.trim();
    if s.len() == 26 && s.is_ascii() && parse_uuid(s).is_none() {
        let value = parse_ulid(s)?;
        return Ok(format!("Type: ULID\n{}\nRandomness: {:020X}", timestamp_line((value >> 80) as u64), value & ((1u128 << 80) - 1)));
    }
    let bytes = parse_uuid(s).ok_or("Not a UUID or ULID")?;
    let value = u128::from_be_bytes(bytes);
    if value == 0 { return Ok("Type: Nil UUID (all zeros)".into()); }
    if value == u128::MAX { return Ok("Type: Max UUID (all ones)".into()); }
    let version = bytes[6] >> 4;
    let variant = match bytes[8] {
        b if b & 0x80 == 0 => "NCS (reserved)",
        b if b & 0xc0 == 0x80 => "RFC 9562 / 4122",
        b if b & 0xe0 == 0xc0 => "Microsoft (reserved)",
        _ => "Future (reserved)",
    };
    let kind = match version {
        1 => "time-based (Gregorian)", 2 => "DCE security", 3 => "name-based (MD5)", 4 => "random",
        5 => "name-based (SHA-1)", 6 => "time-based, sortable", 7 => "Unix time, sortable", 8 => "custom", _ => "unknown",
    };
    let mut lines = vec!["Type: UUID".to_owned(), format!("Version: {version} ({kind})"), format!("Variant: {variant}")];
    if version == 7 {
        let mut millis = [0u8; 8];
        millis[2..].copy_from_slice(&bytes[..6]);
        lines.push(timestamp_line(u64::from_be_bytes(millis)));
    }
    lines.push(format!("Canonical: {}", format_uuid(bytes, Format::default())));
    Ok(lines.join("\n"))
}

/// Decodes each non-empty line; several identifiers get one section each.
fn decode_all(input: &str) -> Result<String, String> {
    let ids: Vec<&str> = input.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    match ids.as_slice() {
        [] => Ok(String::new()),
        [one] => decode_identifier(one),
        many => Ok(many.iter().map(|id| match decode_identifier(id) {
            Ok(details) => format!("ID: {id}\n{details}"),
            Err(e) => format!("ID: {id}\nError: {e}"),
        }).collect::<Vec<_>>().join("\n\n")),
    }
}

/// `last` is the kind generated last, highlighted as the primary button.
pub struct Identifiers { io: TextIo, count: usize, format: Format, last: Kind }

impl Default for Identifiers {
    fn default() -> Self { Self { io: TextIo { output_syntax: Some(ui::KEY_VALUE), ..TextIo::default() }, count: 1, format: Format::default(), last: Kind::UuidV4 } }
}

impl Identifiers {
    fn decode(&mut self) { self.io.apply(decode_all(&self.io.input)); }

    fn generate(&mut self, kind: Kind) {
        self.last = kind;
        match (0..self.count).map(|_| generate(kind, self.format)).collect::<Result<Vec<_>, _>>() {
            Ok(ids) => { self.io.input = ids.join("\n"); self.decode(); }
            Err(e) => self.io.apply(Err(e)),
        }
    }

    /// Rewrites the UUIDs already in the input with the current format options.
    fn reformat(&mut self) {
        self.io.input = self.io.input.lines()
            .map(|line| parse_uuid(line).map_or_else(|| line.to_owned(), |bytes| format_uuid(bytes, self.format)))
            .collect::<Vec<_>>().join("\n");
        self.decode();
    }
}

impl Tool for Identifiers {
    fn name(&self) -> &'static str { "UUID/ULID Generate/Decode" }
    fn icon(&self) -> &'static str { icon::IDENTIFICATION_CARD }
    fn category(&self) -> Category { Category::Security }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Generate UUID v4 (random), UUID v7 (time-sortable) or ULID, or paste identifiers (one per line) to decode them.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "Identifiers:", |ui| {
                changed |= ui::paste_button(ui, &mut self.io.input, &mut self.io.error);
                if ui::clear_button(ui) { self.io.clear(); }
                ui.separator();
                for (label, kind) in [("UUID v4", Kind::UuidV4), ("UUID v7", Kind::UuidV7), ("ULID", Kind::Ulid)] {
                    if ui.add(ui::icon_button(icon::ARROWS_CLOCKWISE, label, kind == self.last)).on_hover_text(format!("Generate {label}")).clicked() { self.generate(kind); }
                }
            });
            let mut reformat = false;
            left.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("How many").color(theme::MUTED));
                ui.add(egui::DragValue::new(&mut self.count).range(1..=1000));
                ui.add_space(8.0);
                reformat |= ui.checkbox(&mut self.format.uppercase, "Uppercase").changed();
                reformat |= ui.checkbox(&mut self.format.no_hyphens, "No hyphens").changed();
            });
            left.add_space(6.0);
            changed |= ui::fill_editor(left, "input", &mut self.io.input, "Paste UUIDs or ULIDs, one per line…").changed();
            if reformat { self.reformat(); } else if changed { self.decode(); }
            if self.io.output_pane(right, |ui, output| super::reuse_menu(ui, output, "UUID/ULID Generate/Decode")) {
                self.io.input = std::mem::take(&mut self.io.output);
                self.decode();
            }
        });
    }

    /// Generates a UUID v7 and shows what it decodes to.
    fn load_sample(&mut self) {
        self.count = 1;
        self.generate(Kind::UuidV7);
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.io.input = text.trim().to_owned();
        self.decode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANDOM: [u8; 16] = [0xab; 16];

    #[test]
    fn builds_each_kind() {
        let v4 = build(Kind::UuidV4, 0, RANDOM, Format::default());
        assert_eq!(v4, "abababab-abab-4bab-abab-abababababab");
        // RFC 9562 example timestamp: 0x017F22E279B0 ms.
        let v7 = build(Kind::UuidV7, 0x017F_22E2_79B0, RANDOM, Format { uppercase: true, no_hyphens: true });
        assert_eq!(v7, "017F22E279B07BABABABABABABABABAB");
        // The ULID spec example "01ARZ3NDEK…" encodes 1469922850259 ms.
        let ulid = build(Kind::Ulid, 1_469_922_850_259, [0; 16], Format::default());
        assert_eq!(&ulid[..10], "01ARZ3NDEK");
        assert!(generate(Kind::UuidV7, Format::default()).unwrap().chars().nth(14) == Some('7'));
    }

    #[test]
    fn decodes_v7_and_ulid_timestamps() {
        let v7 = decode_identifier("017F22E2-79B0-7CC3-98C4-DC0C0C07398F").unwrap();
        assert!(v7.contains("Version: 7 (Unix time, sortable)"), "{v7}");
        assert!(v7.contains("Timestamp: 2022-02-22T19:22:22.000Z"), "{v7}");
        let ulid = decode_identifier("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        assert!(ulid.contains("Type: ULID\nTimestamp: 2016-07-30T23:54:10.259Z"), "{ulid}");
        let v4 = decode_identifier("{550E8400-E29B-41D4-A716-446655440000}").unwrap();
        assert!(v4.contains("Version: 4 (random)") && v4.contains("Canonical: 550e8400-e29b-41d4-a716-446655440000"), "{v4}");
        assert!(decode_identifier("urn:uuid:550e8400e29b41d4a716446655440000").is_ok());
        assert_eq!(decode_identifier("00000000-0000-0000-0000-000000000000").unwrap(), "Type: Nil UUID (all zeros)");
        assert!(decode_identifier("80000000000000000000000000").is_err());
        assert!(decode_identifier("hello").is_err());
    }

    #[test]
    fn decodes_several_lines() {
        let out = decode_all("550e8400-e29b-41d4-a716-446655440000\n\nnope").unwrap();
        assert!(out.starts_with("ID: 550e8400-e29b-41d4-a716-446655440000\nType: UUID"), "{out}");
        assert!(out.ends_with("ID: nope\nError: Not a UUID or ULID"), "{out}");
    }
}
