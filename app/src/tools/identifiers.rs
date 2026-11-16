//! UUID v4 / ULID generation and decoding.
use eframe::egui;
use super::{text::random_bytes, Category, Tool};
use crate::ui::{self, icon, TextIo};

/// ULID alphabet (Crockford's Base32: no I, L, O, U).
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub struct Identifiers { io: TextIo }

impl Default for Identifiers {
    fn default() -> Self { Self { io: TextIo { output_syntax: Some(ui::KEY_VALUE), ..TextIo::default() } } }
}

impl Tool for Identifiers {
    fn name(&self) -> &'static str { "UUID/ULID Generate/Decode" }
    fn icon(&self) -> &'static str { icon::IDENTIFICATION_CARD }
    fn category(&self) -> Category { Category::Security }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Generate a UUID v4 or a ULID, or paste one and decode it.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            match self.io.input_pane(left, &["Generate UUID", "Generate ULID", "Decode"], "Paste a UUID or ULID…", None) {
                Some(i @ (0 | 1)) => {
                    let result = identifier(i == 1);
                    if let Ok(value) = &result { self.io.input = value.clone(); }
                    self.io.apply(result);
                }
                Some(_) => self.io.apply(decode_identifier(&self.io.input)),
                None => {}
            }
            self.io.output_pane(right);
        });
    }

    /// Generates a UUID and shows what it decodes to.
    fn load_sample(&mut self) {
        match identifier(false) {
            Ok(uuid) => { self.io.apply(decode_identifier(&uuid)); self.io.input = uuid; }
            Err(e) => self.io.apply(Err(e)),
        }
    }
}

/// A random UUID v4, or a ULID (48-bit millisecond timestamp + 80 random bits).
pub fn identifier(generate_ulid: bool) -> Result<String, String> {
    let mut bytes = random_bytes::<16>()?;
    if generate_ulid {
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis();
        if millis >= (1u128 << 48) { return Err("Current time is outside the ULID range".into()); }
        bytes[..6].copy_from_slice(&(millis as u64).to_be_bytes()[2..]);
        let mut value = u128::from_be_bytes(bytes);
        let mut encoded = [b'0'; 26];
        for c in encoded.iter_mut().rev() { *c = CROCKFORD[(value & 31) as usize]; value >>= 5; }
        Ok(String::from_utf8(encoded.to_vec()).unwrap())
    } else {
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let h: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Ok(format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..]))
    }
}

/// Describes a ULID (timestamp, randomness) or a UUID (version, variant).
pub fn decode_identifier(input: &str) -> Result<String, String> {
    let s = input.trim();
    if s.len() == 26 && s.is_ascii() {
        let mut value = 0u128;
        for (i, c) in s.bytes().enumerate() {
            let c = c.to_ascii_uppercase();
            let digit = CROCKFORD.iter().position(|&x| x == c).ok_or("Invalid ULID character")? as u128;
            if i == 0 && digit > 7 { return Err("ULID exceeds 128 bits".into()); }
            value = (value << 5) | digit;
        }
        return Ok(format!("Type: ULID\nTimestamp: {} milliseconds since Unix epoch\nRandomness: {:020X}", value >> 80, value & ((1u128 << 80) - 1)));
    }
    if s.len() != 36 || !s.is_ascii() || ![8, 13, 18, 23].iter().all(|&i| s.as_bytes()[i] == b'-') { return Err("Enter a UUID or ULID".into()); }
    let hex = s.replace('-', "");
    let value = u128::from_str_radix(&hex, 16).map_err(|_| "Invalid UUID")?;
    let bytes = value.to_be_bytes();
    let variant = if bytes[8] & 0x80 == 0 { "NCS" } else if bytes[8] & 0xc0 == 0x80 { "RFC 4122" } else if bytes[8] & 0xe0 == 0xc0 { "Microsoft" } else { "Future" };
    Ok(format!("Type: UUID\nVersion: {}\nVariant: {}", bytes[6] >> 4, variant))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiers_round_trip() {
        let uuid = identifier(false).unwrap();
        assert!(decode_identifier(&uuid).unwrap().contains("Version: 4"));
        let ulid = identifier(true).unwrap();
        assert_eq!(ulid.len(), 26);
        assert!(decode_identifier(&ulid).unwrap().contains("Type: ULID"));
        assert!(decode_identifier("80000000000000000000000000").is_err());
    }
}
