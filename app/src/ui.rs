//! Shared look and widgets: the color theme, fonts and icons, the side-by-side
//! input/output layout and the `TextIo` state used by most tools.
use eframe::egui;

/// Phosphor icons (regular weight), e.g. `icon::COPY`. They are glyphs of the
/// proportional font, so they can be put in any label or button text.
pub use egui_phosphor::regular as icon;

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
    /// Mint-tinted fill of the selected sidebar row.
    pub const ROW_SELECTED: Color32 = Color32::from_rgb(45, 66, 56);
    /// Mint green used for icons, primary buttons and checked boxes.
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
    pub const TEXT_STRONG: Color32 = Color32::from_rgb(240, 240, 240);
    pub const MUTED: Color32 = Color32::from_rgb(166, 170, 169);
    pub const SUBTLE: Color32 = Color32::from_rgb(128, 128, 128);
    pub const ERROR_BG: Color32 = Color32::from_rgb(66, 36, 36);
    pub const ERROR_STROKE: Color32 = Color32::from_rgb(112, 52, 52);
    pub const ERROR_TEXT: Color32 = Color32::from_rgb(255, 170, 160);
    /// Background of highlighted ranges (e.g. regex matches).
    pub const MATCH_BG: Color32 = Color32::from_rgb(40, 92, 66);
    pub const DIFF_ADDED: Color32 = Color32::from_rgb(130, 220, 169);
    pub const DIFF_REMOVED: Color32 = Color32::from_rgb(255, 135, 125);
}

pub use theme::MUTED;

/// Pseudo-language for [`Highlight::syntax`]: "Key: value" reports.
pub const KEY_VALUE: &str = "key-value";

/// Texts longer than this are shown without syntax highlighting, which would
/// make typing sluggish on very large inputs.
const MAX_HIGHLIGHT_BYTES: usize = 200_000;

/// Dark visuals with the app's window background.
pub fn apply_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = theme::WINDOW_BG;
    visuals.window_fill = theme::WINDOW_BG;
    ctx.set_visuals(visuals);
}

/// Uses the macOS system fonts (SF Pro / SF Mono) when present. They are read
/// from disk at startup, never bundled. Apple Symbols is added as a fallback so
/// that ⌘ ⇧ ⌥ render, and the Phosphor icon font is added for `icon::*`.
/// On other systems egui's built-in fonts are kept.
pub fn install_fonts(ctx: &egui::Context) {
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
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
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
pub fn action_button(label: impl Into<String>, primary: bool) -> egui::Button<'static> {
    let (fill, text) = if primary { (theme::ACCENT_STRONG, theme::ON_ACCENT) } else { (theme::BUTTON, theme::TEXT) };
    egui::Button::new(egui::RichText::new(label.into()).size(12.5).color(text)).fill(fill).corner_radius(5)
}

/// An action button with a leading icon, e.g. `icon_button(icon::COPY, "Copy", false)`.
pub fn icon_button(glyph: &str, label: &str, primary: bool) -> egui::Button<'static> {
    action_button(format!("{glyph}  {label}"), primary)
}

/// A compact button showing only an icon; `tooltip` says what it does.
pub fn icon_only(ui: &mut egui::Ui, glyph: &str, tooltip: &str) -> bool {
    let button = egui::Button::new(egui::RichText::new(glyph).size(14.0).color(theme::TEXT))
        .fill(theme::BUTTON).corner_radius(5).min_size(egui::vec2(28.0, 0.0));
    ui.add(button).on_hover_text(tooltip).clicked()
}

/// Icon button that loads a tool's example.
pub fn sample_button(ui: &mut egui::Ui) -> bool { icon_only(ui, icon::SPARKLE, "Load an example") }

/// Icon button that empties a tool.
pub fn clear_button(ui: &mut egui::Ui) -> bool { icon_only(ui, icon::ERASER, "Clear") }

/// Pastes the clipboard's text into `target` (or reports the error) when clicked.
pub fn paste_button(ui: &mut egui::Ui, target: &mut String, error: &mut String) -> bool {
    let clicked = icon_only(ui, icon::CLIPBOARD_TEXT, "Paste from the clipboard");
    if clicked {
        match clipboard_text() { Ok(text) => *target = text, Err(e) => *error = e }
    }
    clicked
}

/// "Copy" button that turns into "Copied" for a moment after a click.
pub fn copy_button(ui: &mut egui::Ui, text: &str) {
    const FEEDBACK: f64 = 1.5;
    let id = ui.id().with("copy-feedback");
    let now = ui.input(|i| i.time);
    // Seconds of "Copied" feedback left, if the button was clicked recently.
    let remaining = ui.data(|d| d.get_temp::<f64>(id)).map(|at| FEEDBACK - (now - at)).filter(|left| *left > 0.0);
    let button = if remaining.is_some() { icon_button(icon::CHECK, "Copied", false) } else { icon_button(icon::COPY, "Copy", false) };
    if ui.add(button).on_hover_text("Copy to the clipboard").clicked() {
        ui.ctx().copy_text(text.to_owned());
        ui.data_mut(|d| d.insert_temp(id, now));
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(FEEDBACK));
    } else if let Some(left) = remaining {
        // Repaint once more when the feedback should disappear.
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(left));
    }
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

/// How an editor colors its text.
pub enum Highlight<'a> {
    Plain,
    /// Syntax highlighting: a syntect language name or file extension (`"json"`, `"sql"`, `"rs"`…).
    Syntax(&'a str),
    /// Byte ranges drawn on a green background (e.g. regex matches).
    Ranges(&'a [std::ops::Range<usize>]),
    /// Diff output: `+` lines green, `-` lines red, unchanged lines muted.
    Diff,
    /// "Key: value" lines: keys muted, values bright.
    KeyValue,
}

impl<'a> Highlight<'a> {
    /// Highlighting for an optional language name; [`KEY_VALUE`] selects [`Highlight::KeyValue`].
    pub fn syntax(language: Option<&'a str>) -> Self {
        match language {
            None => Highlight::Plain,
            Some(KEY_VALUE) => Highlight::KeyValue,
            Some(language) => Highlight::Syntax(language),
        }
    }
}

/// A scrollable monospace editor that fills the remaining space of `ui`, with
/// a line/character count underneath. `id` must be unique within the tool so
/// each editor keeps its own scroll position.
pub fn fill_editor(ui: &mut egui::Ui, id: &str, text: &mut String, hint: &str) -> egui::Response {
    code_editor(ui, id, text, hint, Highlight::Plain)
}

/// Like [`fill_editor`], with highlighting.
pub fn code_editor(ui: &mut egui::Ui, id: &str, text: &mut String, hint: &str, highlight: Highlight) -> egui::Response {
    const STATUS_HEIGHT: f32 = 18.0;
    let highlight = if text.len() > MAX_HIGHLIGHT_BYTES { Highlight::Plain } else { highlight };
    let response = editor_frame().show(ui, |ui| {
        let size = ui.available_size() - egui::vec2(0.0, STATUS_HEIGHT);
        egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).max_height(size.y).show(ui, |ui| {
            let editor = egui::TextEdit::multiline(text).frame(false).font(egui::TextStyle::Monospace)
                .min_size(size).desired_width(f32::INFINITY).hint_text(hint);
            match highlight {
                Highlight::Plain => ui.add(editor),
                Highlight::Syntax(language) => {
                    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut job = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, string, language);
                        // egui_extras 0.31 underlines everything italic; keep the italics only.
                        for section in &mut job.sections { section.format.underline = egui::Stroke::NONE; }
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|fonts| fonts.layout_job(job))
                    };
                    ui.add(editor.layouter(&mut layouter))
                }
                Highlight::Ranges(ranges) => {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut job = ranges_job(ui, string, ranges);
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|fonts| fonts.layout_job(job))
                    };
                    ui.add(editor.layouter(&mut layouter))
                }
                Highlight::Diff | Highlight::KeyValue => {
                    let diff = matches!(highlight, Highlight::Diff);
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut job = if diff { diff_job(ui, string) } else { key_value_job(ui, string) };
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|fonts| fonts.layout_job(job))
                    };
                    ui.add(editor.layouter(&mut layouter))
                }
            }
        }).inner
    }).inner;
    ui.horizontal(|ui| {
        ui.set_height(STATUS_HEIGHT);
        if !text.is_empty() {
            let lines = text.lines().count().max(1);
            let chars = text.chars().count();
            ui.label(egui::RichText::new(format!("{lines} {} · {chars} {}", plural(lines, "line"), plural(chars, "char"))).small().color(theme::SUBTLE));
        }
    });
    response
}

/// Lays out `text` with `ranges` on a highlighted background. Ranges that do not
/// fit the text (it may have changed since they were computed) are skipped.
fn ranges_job(ui: &egui::Ui, text: &str, ranges: &[std::ops::Range<usize>]) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let normal = egui::TextFormat::simple(font.clone(), theme::TEXT);
    let marked = egui::TextFormat { background: theme::MATCH_BG, ..egui::TextFormat::simple(font, theme::TEXT_STRONG) };
    let mut job = egui::text::LayoutJob::default();
    let mut pos = 0;
    for range in ranges {
        let fits = range.start >= pos && range.end <= text.len() && text.is_char_boundary(range.start) && text.is_char_boundary(range.end);
        if !fits { continue; }
        job.append(&text[pos..range.start], 0.0, normal.clone());
        job.append(&text[range.clone()], 0.0, marked.clone());
        pos = range.end;
    }
    job.append(&text[pos..], 0.0, normal);
    job
}

/// Colors each line of a diff by its first character.
fn diff_job(ui: &egui::Ui, text: &str) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    for line in text.split_inclusive('\n') {
        let color = match line.as_bytes().first() { Some(b'+') => theme::DIFF_ADDED, Some(b'-') => theme::DIFF_REMOVED, _ => MUTED };
        job.append(line, 0.0, egui::TextFormat::simple(font.clone(), color));
    }
    job
}

/// Colors keys muted and values bright, line by line: `Key: value`,
/// `key = value`, and `Heading:` lines whose value follows on the next lines.
fn key_value_job(ui: &egui::Ui, text: &str) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let key = egui::TextFormat::simple(font.clone(), MUTED);
    let value = egui::TextFormat::simple(font, theme::TEXT_STRONG);
    let mut job = egui::text::LayoutJob::default();
    for line in text.split_inclusive('\n') {
        let split = line.find(": ").map(|at| at + 1).or_else(|| line.find(" = ").map(|at| at + 2));
        match split {
            Some(at) => {
                job.append(&line[..at], 0.0, key.clone());
                job.append(&line[at..], 0.0, value.clone());
            }
            None if line.trim_end().ends_with(':') => job.append(line, 0.0, key.clone()),
            None => job.append(line, 0.0, value.clone()),
        }
    }
    job
}

fn plural(n: usize, word: &str) -> String { if n == 1 { word.to_owned() } else { format!("{word}s") } }

/// A read-only area that fills the remaining space, for previews drawn with widgets.
pub fn preview_frame(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    editor_frame().show(ui, |ui| {
        let size = ui.available_size();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_min_size(size);
            add(ui);
        });
    });
}

/// Shows `error` in a red banner with a warning icon, or nothing when it is empty.
pub fn error_label(ui: &mut egui::Ui, error: &str) {
    if error.is_empty() { return; }
    egui::Frame::new().fill(theme::ERROR_BG).stroke(egui::Stroke::new(1.0_f32, theme::ERROR_STROKE))
        .corner_radius(6).inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(icon::WARNING_CIRCLE).color(theme::ERROR_TEXT));
                ui.label(egui::RichText::new(error).color(theme::ERROR_TEXT));
            });
        });
    ui.add_space(6.0);
}

/// Reads text from the system clipboard. egui only exposes clipboard text through
/// paste events, so the "Clipboard" buttons go through `arboard` directly.
pub fn clipboard_text() -> Result<String, String> {
    arboard::Clipboard::new().and_then(|mut c| c.get_text()).map_err(|e| format!("Clipboard: {e}"))
}

/// Input, output and error state shared by text-in/text-out tools.
#[derive(Default)]
pub struct TextIo {
    pub input: String,
    pub output: String,
    pub error: String,
    /// Syntax highlighting of each pane (`None` = plain text).
    pub input_syntax: Option<&'static str>,
    pub output_syntax: Option<&'static str>,
}

impl TextIo {
    /// Stores a transform result: the value goes to the output, an error replaces it.
    pub fn apply(&mut self, result: Result<String, String>) {
        match result { Ok(value) => { self.output = value; self.error.clear(); } Err(error) => { self.output.clear(); self.error = error; } }
    }

    /// Empties the text but keeps the highlighting settings.
    pub fn clear(&mut self) {
        self.input.clear();
        self.output.clear();
        self.error.clear();
    }

    /// Draws the input pane (toolbar + editor) and returns the index of the
    /// action button that was clicked this frame. The first action is the
    /// primary one. With a `sample`, a "Sample" button loads it and runs the
    /// primary action.
    pub fn input_pane(&mut self, ui: &mut egui::Ui, actions: &[&str], hint: &str, sample: Option<&str>) -> Option<usize> {
        let clicked = toolbar(ui, "Input:", |ui| {
            paste_button(ui, &mut self.input, &mut self.error);
            let mut clicked = None;
            if let Some(sample) = sample {
                if sample_button(ui) {
                    self.input = sample.to_owned();
                    clicked = Some(0);
                }
            }
            if clear_button(ui) { self.clear(); }
            ui.separator();
            for (i, label) in actions.iter().enumerate() {
                if ui.add(action_button(*label, i == 0)).clicked() { clicked = Some(i); }
            }
            clicked
        });
        code_editor(ui, "input", &mut self.input, hint, Highlight::syntax(self.input_syntax));
        clicked
    }

    /// Draws the output pane: toolbar with Copy, the error if any, then the result.
    pub fn output_pane(&mut self, ui: &mut egui::Ui) {
        toolbar(ui, "Output:", |ui| copy_button(ui, &self.output));
        error_label(ui, &self.error);
        code_editor(ui, "output", &mut self.output, "", Highlight::syntax(self.output_syntax));
    }
}
