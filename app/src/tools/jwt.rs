//! JWT Debugger: decodes a token's header and payload, shows its time claims as
//! dates with a validity status, and verifies HMAC (HS256/384/512) signatures.
use std::ops::Range;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use eframe::egui;
use hmac::{Hmac, Mac};
use serde_json::Value;
use super::{encoding::decode_base64, unix_time::{relative, DATE_TIME_FORMAT}, Category, Tool};
use crate::ui::{self, icon, theme, Highlight};

/// The public example token from jwt.io ("John Doe"), signed with its demo
/// secret below. Neither is a credential, hence the secret-scanner exceptions.
const SAMPLE_JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"; // gitleaks:allow
/// The HMAC secret that signs [`SAMPLE_JWT`].
const SAMPLE_SECRET: &str = "your-256-bit-secret"; // gitleaks:allow

/// Header color (red), as on jwt.io: in the token and on the "Header" title.
const HEADER_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 120, 120);
/// Payload color (purple), as on jwt.io.
const PAYLOAD_COLOR: egui::Color32 = egui::Color32::from_rgb(205, 140, 255);
/// Signature color (blue), as on jwt.io.
const SIGNATURE_COLOR: egui::Color32 = egui::Color32::from_rgb(80, 200, 230);

/// A decoded token.
#[derive(Debug)]
struct Decoded {
    header: Value,
    payload: Value,
    /// `header.payload` as typed (still Base64URL): the bytes the signature covers.
    signing_input: String,
    signature: Vec<u8>,
    /// Byte ranges of header, payload and signature in the text as typed (for coloring).
    parts: [Range<usize>; 3],
}

/// Decodes one segment: URL-safe Base64 without padding (a stray `=` is tolerated). `name`
/// ("header", "payload"…) is used in the error message.
fn decode_part(part: &str, name: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD.decode(part.trim_end_matches('=')).map_err(|e| format!("The {name} is not valid Base64URL: {e}"))
}

/// Splits a token into its three dot-separated segments and decodes them.
/// Whitespace around the token is ignored.
fn decode(text: &str) -> Result<Decoded, String> {
    // Leading whitespace shifts the ranges in `parts`, which index the text as typed.
    let offset = text.len() - text.trim_start().len();
    let token = text.trim();
    let pieces: Vec<&str> = token.split('.').collect();
    let [header, payload, signature] = pieces.as_slice() else {
        return Err("A JWT has three parts separated by dots: header.payload.signature".into());
    };
    let json = |part: &str, name: &str| -> Result<Value, String> {
        serde_json::from_slice(&decode_part(part, name)?).map_err(|e| format!("The {name} is not valid JSON: {e}"))
    };
    let header_end = offset + header.len();
    let payload_end = header_end + 1 + payload.len();
    Ok(Decoded {
        header: json(header, "header")?,
        payload: json(payload, "payload")?,
        signing_input: format!("{header}.{payload}"),
        signature: decode_part(signature, "signature")?,
        parts: [offset..header_end, header_end + 1..payload_end, payload_end + 1..payload_end + 1 + signature.len()],
    })
}

/// Checks an HMAC signature in constant time. Other algorithms need a public
/// key and are reported as unsupported.
fn verify(decoded: &Decoded, secret: &[u8]) -> Result<bool, String> {
    /// Whether `signature` is the HMAC of `data` with `secret`, using the MAC type `M`.
    fn check<M: Mac + hmac::digest::KeyInit>(secret: &[u8], data: &[u8], signature: &[u8]) -> bool {
        let mut mac = <M as hmac::digest::KeyInit>::new_from_slice(secret).expect("HMAC accepts keys of any length");
        mac.update(data);
        mac.verify_slice(signature).is_ok()
    }
    let data = decoded.signing_input.as_bytes();
    match decoded.header.get("alg").and_then(Value::as_str) {
        Some("HS256") => Ok(check::<Hmac<sha2::Sha256>>(secret, data, &decoded.signature)),
        Some("HS384") => Ok(check::<Hmac<sha2::Sha384>>(secret, data, &decoded.signature)),
        Some("HS512") => Ok(check::<Hmac<sha2::Sha512>>(secret, data, &decoded.signature)),
        Some("none") => Err("Unsigned token (alg \"none\"): there is no signature to verify".into()),
        Some(other) => Err(format!("{other} signatures need a public key: only HS256, HS384 and HS512 can be verified here")),
        None => Err("The header has no \"alg\"".into()),
    }
}

/// Whether the token can be used at `now`, from its `exp` and `nbf` claims.
#[derive(Debug, PartialEq)]
enum Validity {
    /// No `exp` claim.
    NoExpiry,
    /// Usable now; holds the `exp` time.
    Valid(DateTime<Utc>),
    /// Holds the `exp` time, which has passed.
    Expired(DateTime<Utc>),
    /// Holds the `nbf` time, still in the future.
    NotYet(DateTime<Utc>),
}

/// A time claim (`exp`, `nbf`, `iat`) as an instant. JWT times are seconds
/// since the epoch and may have a fraction, which is dropped.
fn claim_time(payload: &Value, claim: &str) -> Option<DateTime<Utc>> {
    let seconds = payload.get(claim)?.as_f64()?;
    DateTime::from_timestamp(seconds.floor() as i64, 0)
}

/// The token's status at `now`. A future `nbf` wins over an expired `exp`.
fn validity(payload: &Value, now: DateTime<Utc>) -> Validity {
    if let Some(nbf) = claim_time(payload, "nbf").filter(|nbf| *nbf > now) { return Validity::NotYet(nbf); }
    match claim_time(payload, "exp") {
        Some(exp) if exp <= now => Validity::Expired(exp),
        Some(exp) => Validity::Valid(exp),
        None => Validity::NoExpiry,
    }
}

/// The JWT Debugger tool.
#[derive(Default)]
pub struct Jwt {
    token: String,
    /// HMAC secret; empty = do not verify.
    secret: String,
    /// Decode `secret` from Base64 before use, for issuers that store binary keys that way.
    secret_is_base64: bool,
    /// `None` when the token is empty or invalid (see `error`).
    decoded: Option<Decoded>,
    /// Header as pretty JSON.
    header_json: String,
    /// Payload as pretty JSON.
    payload_json: String,
    /// The colored ranges of the token, see [`Decoded::parts`].
    spans: Vec<(Range<usize>, egui::Color32)>,
    /// Verification result: `None` without a secret, `Err` when it cannot be
    /// checked (algorithm without HMAC, invalid Base64 secret).
    signature: Option<Result<bool, String>>,
    error: String,
}

impl Jwt {
    /// Decodes the token and, when a secret is entered, verifies the signature.
    fn update(&mut self) {
        self.decoded = None;
        self.spans.clear();
        self.signature = None;
        self.error.clear();
        if self.token.trim().is_empty() { return; }
        match decode(&self.token) {
            Ok(decoded) => {
                self.header_json = serde_json::to_string_pretty(&decoded.header).unwrap_or_default();
                self.payload_json = serde_json::to_string_pretty(&decoded.payload).unwrap_or_default();
                self.spans = decoded.parts.iter().cloned().zip([HEADER_COLOR, PAYLOAD_COLOR, SIGNATURE_COLOR]).collect();
                if !self.secret.is_empty() {
                    let secret = if self.secret_is_base64 { decode_base64(&self.secret) } else { Ok(self.secret.as_bytes().to_vec()) };
                    self.signature = Some(secret.and_then(|secret| verify(&decoded, &secret)));
                }
                self.decoded = Some(decoded);
            }
            Err(e) => self.error = e,
        }
    }

    /// Status badges, header, payload and time claims.
    fn details(&self, ui: &mut egui::Ui) {
        let Some(decoded) = &self.decoded else {
            ui.label(egui::RichText::new("Paste a token on the left to see its contents.").color(theme::SUBTLE));
            return;
        };
        let now = Utc::now();
        ui.horizontal_wrapped(|ui| {
            match validity(&decoded.payload, now) {
                Validity::Valid(exp) => ui::badge(ui, icon::CHECK_CIRCLE, &format!("Valid, expires {}", relative(exp - now)), theme::DIFF_ADDED),
                Validity::Expired(exp) => ui::badge(ui, icon::X_CIRCLE, &format!("Expired {}", relative(exp - now)), theme::DIFF_REMOVED),
                Validity::NotYet(nbf) => ui::badge(ui, icon::CLOCK, &format!("Not valid until {}", relative(nbf - now)), theme::WARNING),
                Validity::NoExpiry => ui::badge(ui, icon::CLOCK, "No expiry (no exp claim)", theme::MUTED),
            }
            match &self.signature {
                Some(Ok(true)) => ui::badge(ui, icon::SHIELD_CHECK, "Signature verified", theme::DIFF_ADDED),
                Some(Ok(false)) => ui::badge(ui, icon::SHIELD_WARNING, "Invalid signature", theme::DIFF_REMOVED),
                Some(Err(_)) => ui::badge(ui, icon::SHIELD_WARNING, "Signature not checked", theme::WARNING),
                None => ui::badge(ui, icon::LOCK_KEY, "Enter the secret to verify", theme::MUTED),
            }
        });
        if let Some(Err(reason)) = &self.signature { ui.label(egui::RichText::new(reason).small().color(theme::WARNING)); }
        for (title, json, color) in [("Header", &self.header_json, HEADER_COLOR), ("Payload", &self.payload_json, PAYLOAD_COLOR)] {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(title).strong().color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui::copy_button(ui, json));
            });
            ui::code_view(ui, json, "json");
        }
        let times: Vec<(&str, &str, DateTime<Utc>)> = [("exp", "Expires"), ("nbf", "Not before"), ("iat", "Issued")].into_iter()
            .filter_map(|(claim, label)| claim_time(&decoded.payload, claim).map(|t| (claim, label, t))).collect();
        if !times.is_empty() {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Time claims").strong());
            egui::Grid::new("jwt-times").num_columns(3).spacing([12.0, 4.0]).show(ui, |ui| {
                for (claim, label, time) in times {
                    ui.label(egui::RichText::new(format!("{label} ({claim})")).color(theme::MUTED));
                    ui.label(egui::RichText::new(time.with_timezone(&chrono::Local).format(DATE_TIME_FORMAT).to_string()).monospace());
                    ui.label(egui::RichText::new(relative(time - now)).color(theme::MUTED));
                    ui.end_row();
                }
            });
        }
    }
}

impl Tool for Jwt {
    fn name(&self) -> &'static str { "JWT Debugger" }
    fn icon(&self) -> &'static str { icon::KEY }
    fn category(&self) -> Category { Category::Security }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // Keeps "expires in …" current.
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        ui::description(ui, "Decodes a JSON Web Token. The signature is verified when you enter the secret (HS256, HS384, HS512).");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "Token:", |ui| {
                changed |= ui::paste_button(ui, &mut self.token, &mut self.error);
                if ui::sample_button(ui) { self.load_sample(); }
                if ui::clear_button(ui) { *self = Self::default(); }
            });
            let height = (left.available_height() * 0.6).max(120.0);
            left.allocate_ui(egui::vec2(left.available_width(), height), |ui| {
                changed |= ui::code_editor(ui, "token", &mut self.token, "eyJhbGciOi…", Highlight::Spans(&self.spans)).changed();
            });
            left.add_space(8.0);
            ui::toolbar(left, "Secret:", |ui| {
                changed |= ui.checkbox(&mut self.secret_is_base64, "Base64-encoded").changed();
            });
            changed |= ui::editor_frame().show(left, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.secret).frame(false).font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY).hint_text("HMAC secret, to verify the signature"))
            }).inner.changed();
            if changed { self.update(); }
            ui::toolbar(right, "Decoded:", |_| {});
            ui::error_label(right, &self.error);
            ui::preview_frame(right, |ui| self.details(ui));
        });
    }

    fn load_sample(&mut self) {
        self.token = SAMPLE_JWT.into();
        self.secret = SAMPLE_SECRET.into();
        self.secret_is_base64 = false;
        self.update();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.token = text.trim().to_owned();
        self.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    #[test]
    fn decodes_and_colors_parts() {
        let text = format!("  {SAMPLE_JWT}\n");
        let decoded = decode(&text).unwrap();
        assert_eq!(decoded.payload["name"], "John Doe");
        assert_eq!(decoded.header["alg"], "HS256");
        assert_eq!(&text[decoded.parts[0].clone()], "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
        assert!(text[decoded.parts[2].clone()].starts_with("SflKx"));
        assert!(decode("abc.def").is_err());
        assert!(decode("e30.bm90IGpzb24.").unwrap_err().contains("payload is not valid JSON"));
    }

    #[test]
    fn verifies_hmac_signatures() {
        let decoded = decode(SAMPLE_JWT).unwrap();
        assert_eq!(verify(&decoded, SAMPLE_SECRET.as_bytes()), Ok(true));
        assert_eq!(verify(&decoded, b"wrong secret"), Ok(false));
        let rs256 = decode("eyJhbGciOiJSUzI1NiJ9.e30.c2ln").unwrap();
        assert!(verify(&rs256, b"x").unwrap_err().contains("public key"));
        let unsigned = decode("eyJhbGciOiJub25lIn0.e30.").unwrap();
        assert!(verify(&unsigned, b"x").unwrap_err().starts_with("Unsigned token"));
    }

    #[test]
    fn reports_validity() {
        let now = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let payload = |json: &str| serde_json::from_str::<Value>(json).unwrap();
        assert_eq!(validity(&payload(r#"{"exp": 1700003600}"#), now), Validity::Valid(now + TimeDelta::hours(1)));
        assert_eq!(validity(&payload(r#"{"exp": 1699999999}"#), now), Validity::Expired(now - TimeDelta::seconds(1)));
        assert_eq!(validity(&payload(r#"{"nbf": 1700000060, "exp": 1700003600}"#), now), Validity::NotYet(now + TimeDelta::minutes(1)));
        assert_eq!(validity(&payload(r#"{"sub": "x"}"#), now), Validity::NoExpiry);
    }
}
