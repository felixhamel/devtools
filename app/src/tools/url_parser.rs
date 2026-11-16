//! URL Parser: splits a URL into editable components and a table of query
//! parameters; any edit rebuilds the URL, and editing the URL updates them.
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon, theme};

const SAMPLE: &str = "https://ada:secret@api.example.com:8443/v1/search?q=rust%20egui&page=2&sort=desc#results";

/// The editable parts of a URL. Query values are decoded; `port` is empty
/// when the scheme's default port is used.
#[derive(Clone, Default, PartialEq, Debug)]
struct Parts {
    scheme: String,
    username: String,
    password: String,
    host: String,
    port: String,
    path: String,
    params: Vec<(String, String)>,
    fragment: String,
}

fn parse(input: &str) -> Result<(Parts, Option<u16>), String> {
    let url = url::Url::parse(input.trim()).map_err(|e| format!("Invalid URL: {e}"))?;
    if url.cannot_be_a_base() { return Err("Only URLs with a host can be edited here (e.g. https://…)".into()); }
    let parts = Parts {
        scheme: url.scheme().into(),
        username: url.username().into(),
        password: url.password().unwrap_or_default().into(),
        host: url.host_str().unwrap_or_default().into(),
        port: url.port().map(|p| p.to_string()).unwrap_or_default(),
        path: url.path().into(),
        params: url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect(),
        fragment: url.fragment().unwrap_or_default().into(),
    };
    Ok((parts, url.port_or_known_default()))
}

/// Query keys and values are percent-encoded (`%20` for spaces, not `+`).
fn encode(text: &str) -> String { percent_encoding::utf8_percent_encode(text, super::encoding::COMPONENT).to_string() }

fn build(parts: &Parts) -> Result<String, String> {
    let mut url = url::Url::parse(&format!("{}://{}", parts.scheme, parts.host)).map_err(|e| format!("Invalid scheme or host: {e}"))?;
    url.set_username(&parts.username).map_err(|_| "This URL cannot have a username")?;
    url.set_password(Some(parts.password.as_str()).filter(|p| !p.is_empty())).map_err(|_| "This URL cannot have a password")?;
    let port = match parts.port.trim() {
        "" => None,
        p => Some(p.parse::<u16>().map_err(|_| "The port must be a number from 0 to 65535")?),
    };
    url.set_port(port).map_err(|_| "This URL cannot have a port")?;
    url.set_path(&parts.path);
    let query: Vec<String> = parts.params.iter().filter(|(k, v)| !k.is_empty() || !v.is_empty())
        .map(|(k, v)| format!("{}={}", encode(k), encode(v))).collect();
    url.set_query(Some(query.join("&")).filter(|q| !q.is_empty()).as_deref());
    url.set_fragment(Some(parts.fragment.as_str()).filter(|f| !f.is_empty()));
    Ok(url.to_string())
}

#[derive(Default)]
pub struct UrlParser { input: String, error: String, parts: Option<Parts>, default_port: Option<u16> }

impl UrlParser {
    /// The URL changed: re-read its parts.
    fn parse_input(&mut self) {
        self.error.clear();
        if self.input.trim().is_empty() { self.parts = None; return; }
        match parse(&self.input) {
            Ok((parts, default_port)) => { self.parts = Some(parts); self.default_port = default_port; }
            Err(e) => { self.parts = None; self.error = e; }
        }
    }

    /// A part changed: rebuild the URL (and keep the parts in its normalized form).
    fn rebuild(&mut self) {
        let Some(parts) = &self.parts else { return };
        match build(parts) {
            Ok(url) => { self.input = url; self.error.clear(); self.parse_input(); }
            Err(e) => self.error = e,
        }
    }

    /// Text fields for scheme, credentials, host, port, path and fragment.
    fn components(&mut self, ui: &mut egui::Ui) -> bool {
        let default_port = self.default_port.map(|p| format!("{p} (default)")).unwrap_or_default();
        let Some(p) = &mut self.parts else { return false };
        let mut changed = false;
        egui::Grid::new("url-parts").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            for (label, value, hint) in [
                ("Scheme", &mut p.scheme, ""), ("Username", &mut p.username, ""), ("Password", &mut p.password, ""),
                ("Host", &mut p.host, ""), ("Port", &mut p.port, default_port.as_str()), ("Path", &mut p.path, "/"), ("Fragment", &mut p.fragment, ""),
            ] {
                ui.label(egui::RichText::new(label).color(theme::MUTED));
                // Only rebuild when the field is left or Enter is pressed, so typing is not interrupted.
                let response = ui.add(egui::TextEdit::singleline(value).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).hint_text(hint));
                changed |= response.lost_focus() || (response.changed() && !response.has_focus());
                ui.end_row();
            }
        });
        changed
    }

    /// The query parameters as an editable table.
    fn params(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(p) = &mut self.parts else {
            ui.label(egui::RichText::new("Paste a URL to see its parameters.").color(theme::SUBTLE));
            return false;
        };
        let mut changed = false;
        let mut remove = None;
        // Measured before the grid, and forced with min_size: a grid caps each cell at
        // its column's width from the previous frame, so fields could never grow.
        let width = (ui.available_width() - 60.0).max(120.0);
        egui::Grid::new("url-params").num_columns(3).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(egui::RichText::new("Key").small().color(theme::SUBTLE));
            ui.label(egui::RichText::new("Value (decoded)").small().color(theme::SUBTLE));
            ui.end_row();
            for (i, (key, value)) in p.params.iter_mut().enumerate() {
                let k = ui.add(egui::TextEdit::singleline(key).font(egui::TextStyle::Monospace).min_size(egui::vec2(width * 0.35, 0.0)));
                let v = ui.add(egui::TextEdit::singleline(value).font(egui::TextStyle::Monospace).min_size(egui::vec2(width * 0.65, 0.0)));
                changed |= k.changed() || v.changed();
                if ui::icon_only(ui, icon::TRASH, "Remove this parameter") { remove = Some(i); }
                ui.end_row();
            }
        });
        if let Some(i) = remove { p.params.remove(i); changed = true; }
        ui.add_space(6.0);
        if ui.add(ui::icon_button(icon::PLUS, "Add parameter", false)).clicked() { p.params.push(("key".into(), "value".into())); changed = true; }
        changed
    }
}

impl Tool for UrlParser {
    fn name(&self) -> &'static str { "URL Parser" }
    fn icon(&self) -> &'static str { icon::LINK }
    fn category(&self) -> Category { Category::Data }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Edit any part of the URL or its parameters: the URL is rebuilt as you go.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            let reuse = ui::toolbar(left, "URL:", |ui| {
                changed |= ui::paste_button(ui, &mut self.input, &mut self.error);
                if ui::sample_button(ui) { self.input = SAMPLE.into(); changed = true; }
                if ui::clear_button(ui) { self.input.clear(); changed = true; }
                ui.separator();
                ui::copy_button(ui, &self.input);
                super::reuse_menu(ui, &self.input, "URL Parser")
            });
            let height = (left.available_height() * 0.3).max(80.0);
            left.allocate_ui(egui::vec2(left.available_width(), height), |ui| {
                changed |= ui::fill_editor(ui, "url", &mut self.input, "https://example.com/path?key=value").changed();
            });
            if changed || reuse { self.parse_input(); }
            ui::error_label(left, &self.error);
            left.add_space(8.0);
            ui::toolbar(left, "Components:", |_| {});
            if self.components(left) { self.rebuild(); }

            let count = self.parts.as_ref().map_or(0, |p| p.params.len());
            ui::toolbar(right, &format!("Query parameters ({count}):"), |_| {});
            let mut params_changed = false;
            ui::preview_frame(right, |ui| params_changed = self.params(ui));
            if params_changed { self.rebuild(); }
        });
    }

    fn load_sample(&mut self) {
        self.input = SAMPLE.into();
        self.parse_input();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.input = text.trim().to_owned();
        self.parse_input();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_components_and_decoded_params() {
        let (parts, default_port) = parse(SAMPLE).unwrap();
        assert_eq!((parts.scheme.as_str(), parts.username.as_str(), parts.password.as_str()), ("https", "ada", "secret"));
        assert_eq!((parts.host.as_str(), parts.port.as_str(), parts.path.as_str()), ("api.example.com", "8443", "/v1/search"));
        assert_eq!(parts.params[0], ("q".into(), "rust egui".into()));
        assert_eq!(parts.fragment, "results");
        assert_eq!(default_port, Some(8443));
        assert_eq!(parse("https://example.com").unwrap().1, Some(443));
        assert!(parse("mailto:ada@example.com").is_err());
        assert!(parse("not a url").is_err());
    }

    #[test]
    fn rebuilds_after_edits() {
        let (mut parts, _) = parse(SAMPLE).unwrap();
        assert_eq!(build(&parts).unwrap(), SAMPLE);
        parts.params[1].1 = "3".into();
        parts.params.remove(2);
        parts.params.push(("tag".into(), "a&b".into()));
        parts.port.clear();
        parts.password.clear();
        assert_eq!(build(&parts).unwrap(), "https://ada@api.example.com/v1/search?q=rust%20egui&page=3&tag=a%26b#results");
        parts.params.clear();
        parts.fragment.clear();
        assert_eq!(build(&parts).unwrap(), "https://ada@api.example.com/v1/search");
        parts.port = "99999".into();
        assert!(build(&parts).unwrap_err().contains("port"));
    }
}
