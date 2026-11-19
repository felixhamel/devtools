//! String Case Converter: one input, every naming convention, each with its
//! own Copy button.
use eframe::egui;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
use super::{text::{capitalize, words}, Category, Tool};
use crate::ui::{self, icon};

const SAMPLE: &str = "Parse HTTP response — été 2024";

/// Removes accents (é → e) and drops what is left outside ASCII, for slugs.
fn ascii_fold(text: &str) -> String {
    text.nfd().filter(|c| !is_combining_mark(*c)).filter(char::is_ascii).collect()
}

/// Every conversion of `input`, in display order: (name, result).
fn conversions(input: &str) -> Vec<(&'static str, String)> {
    let parts = words(input);
    let title: Vec<String> = parts.iter().map(|w| capitalize(w)).collect();
    let camel = parts.first().cloned().unwrap_or_default() + &title.iter().skip(1).cloned().collect::<String>();
    vec![
        ("camelCase", camel),
        ("PascalCase", title.concat()),
        ("snake_case", parts.join("_")),
        ("CONSTANT_CASE", parts.join("_").to_uppercase()),
        ("kebab-case", parts.join("-")),
        ("slug", words(&ascii_fold(input)).join("-")),
        ("dot.case", parts.join(".")),
        ("path/case", parts.join("/")),
        ("Title Case", title.join(" ")),
        ("Sentence case", capitalize(&parts.join(" "))),
        ("lowercase", input.to_lowercase()),
        ("UPPERCASE", input.to_uppercase()),
    ]
}

#[derive(Default)]
pub struct CaseConverter { input: String, error: String, results: Vec<(&'static str, String)> }

impl CaseConverter {
    fn update(&mut self) {
        self.results = if self.input.trim().is_empty() { Vec::new() } else { conversions(&self.input) };
    }
}

impl Tool for CaseConverter {
    fn name(&self) -> &'static str { "String Case Converter" }
    fn icon(&self) -> &'static str { icon::TEXT_AA }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Words are found in any style (camelCase, snake_case, spaces, acronyms like HTTPServer).");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "Input:", |ui| {
                changed |= ui::paste_button(ui, &mut self.input, &mut self.error);
                if ui::sample_button(ui) { self.load_sample(); }
                if ui::clear_button(ui) { self.input.clear(); changed = true; }
            });
            ui::error_label(left, &self.error);
            changed |= ui::fill_editor(left, "input", &mut self.input, "Text or identifier to convert…").changed();
            if changed { self.update(); }
            ui::toolbar(right, "Conversions:", |_| {});
            if self.results.is_empty() {
                ui::preview_frame(right, |ui| ui::empty_state(ui, icon::TEXT_AA, "Type something on the left."));
            } else {
                let rows: Vec<ui::Property> = self.results.iter().map(|(key, value)| ui::Property::Pair { key, value, indent: false }).collect();
                ui::property_list(right, "conversions", &rows);
            }
        });
    }

    fn load_sample(&mut self) {
        self.input = SAMPLE.into();
        self.update();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.input = text.to_owned();
        self.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(results: &'a [(&str, String)], name: &str) -> &'a str {
        &results.iter().find(|(n, _)| *n == name).unwrap().1
    }

    #[test]
    fn converts_every_style() {
        let r = conversions("parseHTTPResponse body");
        assert_eq!(get(&r, "camelCase"), "parseHttpResponseBody");
        assert_eq!(get(&r, "PascalCase"), "ParseHttpResponseBody");
        assert_eq!(get(&r, "snake_case"), "parse_http_response_body");
        assert_eq!(get(&r, "CONSTANT_CASE"), "PARSE_HTTP_RESPONSE_BODY");
        assert_eq!(get(&r, "kebab-case"), "parse-http-response-body");
        assert_eq!(get(&r, "dot.case"), "parse.http.response.body");
        assert_eq!(get(&r, "path/case"), "parse/http/response/body");
        assert_eq!(get(&r, "Title Case"), "Parse Http Response Body");
        assert_eq!(get(&r, "Sentence case"), "Parse http response body");
    }

    #[test]
    fn slugs_drop_accents_and_symbols() {
        assert_eq!(get(&conversions(SAMPLE), "slug"), "parse-http-response-ete-2024");
        assert_eq!(get(&conversions("Crème brûlée à Montréal!"), "slug"), "creme-brulee-a-montreal");
    }
}
