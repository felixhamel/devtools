//! Shared look and widgets: the color theme, system fonts, the side-by-side
//! input/output layout and the `TextIo` state used by most tools.
use eframe::egui;

/// Every color the app uses, in one place.
pub mod theme {
    use eframe::egui::Color32;

    pub const WINDOW_BG: Color32 = Color32::from_rgb(46, 44, 44);
    pub const SIDEBAR_BG: Color32 = Color32::from_rgb(37, 37, 39);
    pub const SEARCH_BG: Color32 = Color32::from_rgb(31, 33, 32);
    pub const SEARCH_STROKE: Color32 = Color32::from_rgb(72, 80, 75);
    pub const SEARCH_ICON: Color32 = Color32::from_rgb(151, 164, 155);
    pub const ICON: Color32 = Color32::from_rgb(180, 180, 182);
    pub const ROW_HOVER: Color32 = Color32::from_rgb(48, 48, 50);
    pub const ROW_SELECTED: Color32 = Color32::from_rgb(65, 65, 68);
    pub const BADGE_BG: Color32 = Color32::from_rgb(52, 75, 65);
    /// Mint green used for badges, primary buttons and checked boxes.
    pub const ACCENT: Color32 = Color32::from_rgb(130, 220, 169);
    pub const ACCENT_STRONG: Color32 = Color32::from_rgb(119, 201, 153);
    pub const ACCENT_DARK: Color32 = Color32::from_rgb(76, 151, 110);
    pub const ON_ACCENT: Color32 = Color32::from_rgb(20, 38, 28);
    pub const BUTTON: Color32 = Color32::from_rgb(70, 70, 72);
    pub const BUTTON_HOVER: Color32 = Color32::from_rgb(62, 62, 64);
    pub const PILL: Color32 = Color32::from_rgb(58, 58, 58);
    pub const EDITOR_BG: Color32 = Color32::from_rgb(31, 31, 31);
    pub const EDITOR_STROKE: Color32 = Color32::from_rgb(58, 58, 58);
    pub const TEXT: Color32 = Color32::from_rgb(225, 225, 225);
    pub const TEXT_STRONG: Color32 = Color32::from_rgb(230, 230, 230);
    pub const MUTED: Color32 = Color32::from_rgb(166, 170, 169);
    pub const SUBTLE: Color32 = Color32::GRAY;
}

pub use theme::MUTED;

/// Dark visuals with the app's window background.
pub fn apply_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = theme::WINDOW_BG;
    visuals.window_fill = theme::WINDOW_BG;
    ctx.set_visuals(visuals);
}

/// Uses the macOS system fonts (SF Pro / SF Mono) when present. They are read
/// from disk at startup, never bundled. Apple Symbols is added as a fallback so
/// that ⌘ ⇧ ⌥ render. On other systems egui's built-in fonts are kept.
pub fn install_system_fonts(ctx: &egui::Context) {
    const SYMBOLS: &str = "/System/Library/Fonts/Apple Symbols.ttf";
    let mut fonts = egui::FontDefinitions::default();
    let mut add = |name: &str, path: &str, family: egui::FontFamily, preferred: bool| {
        let Ok(bytes) = std::fs::read(path) else { return };
        fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        let list = fonts.families.entry(family).or_default();
        if preferred { list.insert(0, name.into()) } else { list.push(name.into()) }
    };
    add("sf", "/System/Library/Fonts/SFNS.ttf", egui::FontFamily::Proportional, true);
    add("sf-mono", "/System/Library/Fonts/SFNSMono.ttf", egui::FontFamily::Monospace, true);
    add("symbols", SYMBOLS, egui::FontFamily::Proportional, false);
    add("symbols", SYMBOLS, egui::FontFamily::Monospace, false);
    ctx.set_fonts(fonts);
}

/// A muted one-line explanation shown under a tool's title.
pub fn description(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(MUTED));
}

/// The dark rounded box around editors and previews.
pub fn editor_frame() -> egui::Frame {
    egui::Frame::new().fill(theme::EDITOR_BG)
        .stroke(egui::Stroke::new(1.0_f32, theme::EDITOR_STROKE))
        .corner_radius(6).inner_margin(8)
}

/// A compact toolbar button; the primary one is mint green.
pub fn action_button(label: &str, primary: bool) -> egui::Button<'_> {
    let (fill, text) = if primary { (theme::ACCENT_STRONG, theme::ON_ACCENT) } else { (theme::BUTTON, theme::TEXT) };
    egui::Button::new(egui::RichText::new(label).size(12.5).color(text)).fill(fill).corner_radius(5)
}

/// A pane header: a bold label followed by compact buttons, wrapping when narrow.
pub fn toolbar<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(9.0, 3.0);
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(label).strong().color(theme::TEXT));
            ui.add_space(4.0);
            add(ui)
        }).inner
    }).inner;
    ui.add_space(6.0);
    inner
}

/// Two equal panes side by side, filling the remaining space.
pub fn split<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut egui::Ui) -> R) -> R {
    ui.columns(2, |columns| { let (left, right) = columns.split_at_mut(1); add(&mut left[0], &mut right[0]) })
}

/// A scrollable monospace editor that fills the remaining space of `ui`.
/// `id` must be unique within the tool so each editor keeps its own scroll position.
pub fn fill_editor(ui: &mut egui::Ui, id: &str, text: &mut String, hint: &str) -> egui::Response {
    editor_frame().show(ui, |ui| {
        let size = ui.available_size();
        egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(text).frame(false).font(egui::TextStyle::Monospace)
                .min_size(size).desired_width(f32::INFINITY).hint_text(hint))
        }).inner
    }).inner
}

/// Shows `error` in red, or nothing when it is empty.
pub fn error_label(ui: &mut egui::Ui, error: &str) {
    if !error.is_empty() { ui.colored_label(egui::Color32::LIGHT_RED, error); ui.add_space(4.0); }
}

/// Reads text from the system clipboard. egui only exposes clipboard text through
/// paste events, so the "Clipboard" buttons go through `arboard` directly.
pub fn clipboard_text() -> Result<String, String> {
    arboard::Clipboard::new().and_then(|mut c| c.get_text()).map_err(|e| format!("Clipboard: {e}"))
}

/// Input, output and error state shared by text-in/text-out tools.
#[derive(Default)]
pub struct TextIo { pub input: String, pub output: String, pub error: String }

impl TextIo {
    /// Stores a transform result: the value goes to the output, an error replaces it.
    pub fn apply(&mut self, result: Result<String, String>) {
        match result { Ok(value) => { self.output = value; self.error.clear(); } Err(error) => { self.output.clear(); self.error = error; } }
    }

    pub fn clear(&mut self) { *self = Self::default(); }

    /// Draws the input pane (toolbar + editor) and returns the index of the
    /// action button that was clicked this frame. The first action is the primary one.
    pub fn input_pane(&mut self, ui: &mut egui::Ui, actions: &[&str], hint: &str) -> Option<usize> {
        let clicked = toolbar(ui, "Input:", |ui| {
            if ui.add(action_button("Clipboard", false)).clicked() {
                match clipboard_text() { Ok(text) => self.input = text, Err(e) => self.error = e }
            }
            if ui.add(action_button("Clear", false)).clicked() { self.clear(); }
            ui.separator();
            let mut clicked = None;
            for (i, label) in actions.iter().enumerate() {
                if ui.add(action_button(label, i == 0)).clicked() { clicked = Some(i); }
            }
            clicked
        });
        fill_editor(ui, "input", &mut self.input, hint);
        clicked
    }

    /// Draws the output pane: toolbar with Copy, the error if any, then the result.
    pub fn output_pane(&mut self, ui: &mut egui::Ui) {
        toolbar(ui, "Output:", |ui| {
            if ui.add(action_button("Copy", false)).clicked() { ui.ctx().copy_text(self.output.clone()); }
        });
        error_label(ui, &self.error);
        fill_editor(ui, "output", &mut self.output, "");
    }
}
