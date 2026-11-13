//! Shared widgets, fonts and the side-by-side input/output layout used by most tools.
use eframe::egui;

pub const MUTED: egui::Color32 = egui::Color32::from_rgb(166, 170, 169);

/// Use the macOS system fonts when present (they are read at runtime, never bundled).
/// Apple Symbols is a fallback so ⌘ ⇧ ⌥ render.
pub fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut add = |name: &str, path: &str, family: egui::FontFamily, first: bool| {
        let Ok(bytes) = std::fs::read(path) else { return };
        fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        let list = fonts.families.entry(family).or_default();
        if first { list.insert(0, name.into()) } else { list.push(name.into()) }
    };
    add("sf", "/System/Library/Fonts/SFNS.ttf", egui::FontFamily::Proportional, true);
    add("sf-mono", "/System/Library/Fonts/SFNSMono.ttf", egui::FontFamily::Monospace, true);
    add("symbols", "/System/Library/Fonts/Apple Symbols.ttf", egui::FontFamily::Proportional, false);
    add("symbols", "/System/Library/Fonts/Apple Symbols.ttf", egui::FontFamily::Monospace, false);
    ctx.set_fonts(fonts);
}

pub fn description(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(MUTED));
}

pub fn editor_frame() -> egui::Frame {
    egui::Frame::new().fill(egui::Color32::from_rgb(31, 31, 31))
        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(58, 58, 58)))
        .corner_radius(6).inner_margin(8)
}

pub fn action_button(label: &str, primary: bool) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(label).size(12.5).color(if primary {
        egui::Color32::from_rgb(20, 38, 28)
    } else { egui::Color32::from_rgb(225, 225, 225) }))
        .fill(if primary { egui::Color32::from_rgb(119, 201, 153) } else { egui::Color32::from_rgb(70, 70, 72) })
        .corner_radius(5)
}

/// A pane header: a bold label followed by compact buttons, wrapping when narrow.
pub fn toolbar<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(9.0, 3.0);
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(label).strong().color(egui::Color32::from_rgb(220, 220, 220)));
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
pub fn fill_editor(ui: &mut egui::Ui, id: &str, text: &mut String, hint: &str) -> egui::Response {
    editor_frame().show(ui, |ui| {
        let size = ui.available_size();
        egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(text).frame(false).font(egui::TextStyle::Monospace)
                .min_size(size).desired_width(f32::INFINITY).hint_text(hint))
        }).inner
    }).inner
}

pub fn error_label(ui: &mut egui::Ui, error: &str) {
    if !error.is_empty() { ui.colored_label(egui::Color32::LIGHT_RED, error); ui.add_space(4.0); }
}

pub fn clipboard_text() -> Result<String, String> {
    arboard::Clipboard::new().and_then(|mut c| c.get_text()).map_err(|e| format!("Clipboard: {e}"))
}

/// Input, output and error state shared by text-in/text-out tools.
#[derive(Default)]
pub struct TextIo { pub input: String, pub output: String, pub error: String }

impl TextIo {
    pub fn apply(&mut self, result: Result<String, String>) {
        match result { Ok(value) => { self.output = value; self.error.clear(); } Err(error) => { self.output.clear(); self.error = error; } }
    }

    pub fn clear(&mut self) { *self = Self::default(); }

    /// Draws the input pane and returns the index of the action that was clicked.
    /// The first action is the primary one.
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

    pub fn output_pane(&mut self, ui: &mut egui::Ui) {
        toolbar(ui, "Output:", |ui| {
            if ui.add(action_button("Copy", false)).clicked() { ui.ctx().copy_text(self.output.clone()); }
        });
        error_label(ui, &self.error);
        fill_editor(ui, "output", &mut self.output, "");
    }
}
