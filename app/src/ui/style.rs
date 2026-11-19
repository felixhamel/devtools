//! The app-wide egui style: dark visuals with the app's colors, and the
//! macOS system fonts plus the icon font.
use eframe::egui;
use super::theme;

/// Sets egui's dark visuals with the app's window background, mint selection
/// and editor-colored fields. Called once at startup.
pub fn apply_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = theme::WINDOW_BG;
    visuals.window_fill = theme::WINDOW_BG;
    // Mint accent for selected tabs, checked boxes and selected text.
    visuals.selection.bg_fill = theme::ACCENT_DARK;
    visuals.selection.stroke.color = theme::TEXT_STRONG;
    // Single-line fields share the editors' background instead of black.
    visuals.extreme_bg_color = theme::EDITOR_BG;
    ctx.set_visuals(visuals);
}

/// Uses the macOS system fonts (SF Pro / SF Mono) when present. They are read
/// from disk at startup, never bundled. Apple Symbols is added as a fallback so
/// that ⌘ ⇧ ⌥ render, and the Phosphor icon font is added for `icon::*`.
/// On other systems egui's built-in fonts are kept.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Registers the font at `path`, if it exists, in each of `families`: first
    // when `preferred`, otherwise as a fallback after egui's own fonts.
    let mut add = |name: &str, path: &str, families: &[egui::FontFamily], preferred: bool| {
        let Ok(bytes) = std::fs::read(path) else { return };
        fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        for family in families {
            let list = fonts.families.entry(family.clone()).or_default();
            if preferred { list.insert(0, name.into()) } else { list.push(name.into()) }
        }
    };
    add("sf", "/System/Library/Fonts/SFNS.ttf", &[egui::FontFamily::Proportional], true);
    add("sf-mono", "/System/Library/Fonts/SFNSMono.ttf", &[egui::FontFamily::Monospace], true);
    add("symbols", "/System/Library/Fonts/Apple Symbols.ttf", &[egui::FontFamily::Proportional, egui::FontFamily::Monospace], false);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // Icons can also appear next to monospace text.
    fonts.families.entry(egui::FontFamily::Monospace).or_default().push("phosphor".into());
    ctx.set_fonts(fonts);
}
