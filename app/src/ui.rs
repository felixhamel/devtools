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
    pub const WARNING: Color32 = Color32::from_rgb(240, 185, 95);
    /// Line-number gutter and folded lines of the compare views.
    pub const GUTTER: Color32 = Color32::from_rgb(36, 36, 37);
    pub const FOLD: Color32 = Color32::from_rgb(40, 45, 52);
    /// Background of the missing side of a line in the side-by-side compare view.
    pub const FILLER: Color32 = Color32::from_rgb(26, 26, 26);
    /// Pixels that differ in the image compare.
    pub const HOT: Color32 = Color32::from_rgb(255, 64, 129);
    pub const TOAST_BG: Color32 = Color32::from_rgb(52, 56, 54);
    /// Tool cards on the home screen.
    pub const CARD: Color32 = Color32::from_rgb(53, 51, 51);
    pub const CARD_HOVER: Color32 = Color32::from_rgb(62, 61, 61);
    pub const PALETTE_BG: Color32 = Color32::from_rgb(40, 40, 42);
    /// One color per sidebar category (icons on the home screen and in the palette).
    pub const AMBER: Color32 = Color32::from_rgb(240, 185, 95);
    pub const BLUE: Color32 = Color32::from_rgb(120, 170, 255);
    pub const VIOLET: Color32 = Color32::from_rgb(180, 150, 255);
    pub const ROSE: Color32 = Color32::from_rgb(255, 135, 125);
    pub const PINK: Color32 = Color32::from_rgb(255, 120, 190);
    pub const CYAN: Color32 = Color32::from_rgb(110, 210, 230);
}

pub use theme::MUTED;

/// Pseudo-language for [`Highlight::syntax`]: "Key: value" reports.
pub const KEY_VALUE: &str = "key-value";

/// Texts longer than this are shown without syntax highlighting, which would
/// make typing sluggish on very large inputs.
const MAX_HIGHLIGHT_BYTES: usize = 200_000;

/// Inputs up to this size are processed on every keystroke; larger ones wait
/// for a short pause in typing.
const LIVE_IMMEDIATE_BYTES: usize = 50_000;

/// Delays live processing of large inputs until typing pauses.
#[derive(Default)]
pub struct Debounce { pending: bool, since: f64 }

impl Debounce {
    /// Records that the input changed.
    pub fn touch(&mut self, ctx: &egui::Context) {
        self.pending = true;
        self.since = ctx.input(|i| i.time);
    }

    /// Asks for a run on the next frame, e.g. after the input was set from outside the tool.
    pub fn trigger(&mut self) {
        self.pending = true;
        self.since = f64::NEG_INFINITY;
    }

    /// True once after a change: immediately for small inputs, after a pause
    /// for large ones (a repaint is scheduled so the deadline is not missed).
    pub fn due(&mut self, ctx: &egui::Context, input_len: usize) -> bool {
        if !self.pending { return false; }
        let delay = if input_len > LIVE_IMMEDIATE_BYTES { 0.35 } else { 0.0 };
        let wait = self.since + delay - ctx.input(|i| i.time);
        if wait > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
            return false;
        }
        self.pending = false;
        true
    }
}

/// Dark visuals with the app's window background.
pub fn apply_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = theme::WINDOW_BG;
    visuals.window_fill = theme::WINDOW_BG;
    // Mint accent for selected tabs, checked boxes and selected text.
    visuals.selection.bg_fill = theme::ACCENT_DARK;
    visuals.selection.stroke.color = theme::TEXT_STRONG;
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
    // Icons can also appear next to monospace text.
    fonts.families.entry(egui::FontFamily::Monospace).or_default().push("phosphor".into());
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
    copy_with_feedback(ui, text, ui.id().with("copy-feedback"), false);
}

/// Icon-only copy button (a check mark appears after a click). `id` must be
/// unique among the buttons shown together, e.g. the row index in a grid.
pub fn copy_icon(ui: &mut egui::Ui, text: &str, id: impl std::hash::Hash) {
    copy_with_feedback(ui, text, egui::Id::new(("copy-icon", id)), true);
}

fn copy_with_feedback(ui: &mut egui::Ui, text: &str, id: egui::Id, compact: bool) {
    const FEEDBACK: f64 = 1.5;
    let now = ui.input(|i| i.time);
    // Seconds of "Copied" feedback left, if the button was clicked recently.
    let remaining = ui.data(|d| d.get_temp::<f64>(id)).map(|at| FEEDBACK - (now - at)).filter(|left| *left > 0.0);
    let (glyph, label) = if remaining.is_some() { (icon::CHECK, "Copied") } else { (icon::COPY, "Copy") };
    let button = if compact {
        egui::Button::new(egui::RichText::new(glyph).size(13.0).color(if remaining.is_some() { theme::ACCENT } else { theme::TEXT })).fill(theme::BUTTON).corner_radius(5)
    } else {
        icon_button(glyph, label, false)
    };
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
    /// "Key: value" lines: keys muted, values bright.
    KeyValue,
    /// Byte ranges drawn in the given text colors (e.g. the three parts of a JWT).
    Spans(&'a [(std::ops::Range<usize>, egui::Color32)]),
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
                Highlight::Plain => ui.add(editor.text_color(theme::TEXT)),
                Highlight::Syntax(language) => {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut job = syntax_job(ui, string, language);
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
                Highlight::Spans(spans) => {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut job = spans_job(ui, string, spans);
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|fonts| fonts.layout_job(job))
                    };
                    ui.add(editor.layouter(&mut layouter))
                }
                Highlight::KeyValue => {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut job = key_value_job(ui, string);
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

/// Syntax-highlighted layout of `text` (results are cached by egui_extras).
pub fn syntax_job(ui: &egui::Ui, text: &str, language: &str) -> egui::text::LayoutJob {
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
    let mut job = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, text, language);
    // egui_extras 0.31 underlines everything italic; keep the italics only.
    for section in &mut job.sections { section.format.underline = egui::Stroke::NONE; }
    job
}

/// Read-only, selectable, highlighted code sized to its content (indentation is kept).
pub fn code_view(ui: &mut egui::Ui, code: &str, language: &str) {
    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
        let mut job = syntax_job(ui, string, language);
        job.wrap.max_width = wrap_width;
        ui.fonts(|fonts| fonts.layout_job(job))
    };
    let mut text = code;
    ui.add(egui::TextEdit::multiline(&mut text).frame(false).font(egui::TextStyle::Monospace)
        .desired_rows(1).desired_width(f32::INFINITY).layouter(&mut layouter));
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

/// Lays out `text` with each span in its color; text outside spans is muted.
/// Spans that do not fit the text are skipped, as in [`ranges_job`].
fn spans_job(ui: &egui::Ui, text: &str, spans: &[(std::ops::Range<usize>, egui::Color32)]) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    let mut pos = 0;
    for (range, color) in spans {
        let fits = range.start >= pos && range.end <= text.len() && text.is_char_boundary(range.start) && text.is_char_boundary(range.end);
        if !fits { continue; }
        job.append(&text[pos..range.start], 0.0, egui::TextFormat::simple(font.clone(), MUTED));
        job.append(&text[range.clone()], 0.0, egui::TextFormat::simple(font.clone(), *color));
        pos = range.end;
    }
    job.append(&text[pos..], 0.0, egui::TextFormat::simple(font, MUTED));
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

/// A small rounded status pill, e.g. `badge(ui, icon::CHECK_CIRCLE, "Signature verified", theme::DIFF_ADDED)`.
pub fn badge(ui: &mut egui::Ui, glyph: &str, text: &str, color: egui::Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.18)).stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.6)))
        .corner_radius(10).inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| { ui.label(egui::RichText::new(format!("{glyph}  {text}")).color(color)); });
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
    /// The selected action ("mode"): it runs again whenever the input changes.
    pub active: usize,
    /// Pending live update after the input changed.
    pub live: Debounce,
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

    /// Draws the input pane (toolbar + editor). The action buttons act as a
    /// mode switch: the selected one is highlighted and re-runs live as the
    /// input changes. Returns true when the result should be recomputed now
    /// (mode changed, sample loaded, or the input changed).
    pub fn input_pane(&mut self, ui: &mut egui::Ui, actions: &[&str], hint: &str, sample: Option<&str>) -> bool {
        let mut run_now = false;
        toolbar(ui, "Input:", |ui| {
            if paste_button(ui, &mut self.input, &mut self.error) { self.live.touch(ui.ctx()); }
            if let Some(sample) = sample {
                if sample_button(ui) {
                    self.input = sample.to_owned();
                    self.active = 0;
                    run_now = true;
                }
            }
            if clear_button(ui) { self.clear(); }
            ui.separator();
            for (i, label) in actions.iter().enumerate() {
                if ui.add(action_button(*label, i == self.active)).clicked() {
                    self.active = i;
                    run_now = true;
                }
            }
        });
        if code_editor(ui, "input", &mut self.input, hint, Highlight::syntax(self.input_syntax)).changed() {
            self.live.touch(ui.ctx());
        }
        run_now || self.live.due(ui.ctx(), self.input.len())
    }

    /// Draws the output pane: toolbar with Copy and whatever `extra` adds
    /// (it receives the output text), the error if any, then the result.
    pub fn output_pane<R>(&mut self, ui: &mut egui::Ui, extra: impl FnOnce(&mut egui::Ui, &str) -> R) -> R {
        let result = toolbar(ui, "Output:", |ui| {
            copy_button(ui, &self.output);
            extra(ui, &self.output)
        });
        error_label(ui, &self.error);
        code_editor(ui, "output", &mut self.output, "", Highlight::syntax(self.output_syntax));
        result
    }
}

/// Joined toggle buttons of which exactly one is selected, e.g. view modes.
/// Returns true when the user picked another option.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, current: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    egui::Frame::new().fill(theme::SEARCH_BG).stroke(egui::Stroke::new(1.0_f32, theme::EDITOR_STROKE))
        .corner_radius(6).inner_margin(2)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                for (value, label) in options {
                    let selected = *current == *value;
                    let text = egui::RichText::new(*label).size(12.5).color(if selected { theme::ON_ACCENT } else { theme::TEXT });
                    let fill = if selected { theme::ACCENT_STRONG } else { egui::Color32::TRANSPARENT };
                    let button = egui::Button::new(text).fill(fill).stroke(egui::Stroke::NONE).corner_radius(4);
                    if ui.add(button).clicked() && !selected {
                        *current = *value;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Shows the system Open dialog (blocking) and returns the chosen files.
/// `extensions` limits the choice to those types; empty means any file.
pub fn pick_files(title: &str, extensions: &[&str]) -> Vec<std::path::PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if !extensions.is_empty() { dialog = dialog.add_filter("Supported files", extensions); }
    dialog.pick_files().unwrap_or_default()
}

/// Name and bytes of a file dropped on the window.
pub fn read_dropped(file: &egui::DroppedFile) -> Result<(String, Vec<u8>), String> {
    match (&file.path, &file.bytes) {
        (Some(path), _) => read_path(path),
        (None, Some(bytes)) => Ok((file.name.clone(), bytes.to_vec())),
        _ => Err(format!("Cannot read {}", file.name)),
    }
}

/// Name and bytes of a file on disk.
pub fn read_path(path: &std::path::Path) -> Result<(String, Vec<u8>), String> {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned());
    std::fs::read(path).map(|bytes| (name, bytes)).map_err(|e| format!("Cannot read {}: {e}", path.display()))
}

/// While files are dragged over the window, covers `rect` with a dashed
/// "drop here" frame and `text`.
pub fn drop_hint(ctx: &egui::Context, rect: egui::Rect, text: &str) {
    if ctx.input(|i| i.raw.hovered_files.is_empty()) { return; }
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop-hint")));
    painter.rect_filled(rect, 10.0, egui::Color32::from_black_alpha(190));
    let r = rect.shrink(8.0);
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    painter.extend(egui::Shape::dashed_line(&corners, egui::Stroke::new(2.0_f32, theme::ACCENT), 10.0, 7.0));
    painter.text(rect.center() - egui::vec2(0.0, 18.0), egui::Align2::CENTER_CENTER, icon::DOWNLOAD_SIMPLE, egui::FontId::proportional(42.0), theme::ACCENT);
    painter.text(rect.center() + egui::vec2(0.0, 24.0), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(16.0), theme::TEXT_STRONG);
}

/// A message shown for a moment at the bottom of the window.
#[derive(Clone)]
struct Toast { glyph: &'static str, text: String, at: f64 }

fn toast_id() -> egui::Id { egui::Id::new("devutils-toast") }

/// Shows `text` at the bottom of the window for a few seconds (it replaces
/// the previous message, if any).
pub fn toast(ctx: &egui::Context, glyph: &'static str, text: impl Into<String>) {
    let at = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(toast_id(), Toast { glyph, text: text.into(), at }));
}

/// Draws the current toast, sliding it in and fading it out. Called once per frame by the app.
pub fn show_toast(ctx: &egui::Context) {
    const SHOWN: f64 = 2.8;
    const FADE: f64 = 0.25;
    let Some(toast) = ctx.data(|d| d.get_temp::<Toast>(toast_id())) else { return };
    let age = ctx.input(|i| i.time) - toast.at;
    if age > SHOWN {
        ctx.data_mut(|d| d.remove::<Toast>(toast_id()));
        return;
    }
    let opacity = (age / FADE).min((SHOWN - age) / FADE).clamp(0.0, 1.0) as f32;
    let rise = (1.0 - (age / FADE).min(1.0) as f32) * 14.0;
    egui::Area::new(toast_id().with("area")).order(egui::Order::Tooltip).interactable(false)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -28.0 + rise))
        .show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            egui::Frame::new().fill(theme::TOAST_BG).stroke(egui::Stroke::new(1.0_f32, theme::ACCENT_DARK))
                .corner_radius(18).inner_margin(egui::Margin::symmetric(16, 9))
                .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: egui::Color32::from_black_alpha(110) })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(toast.glyph).size(15.0).color(theme::ACCENT));
                        ui.label(egui::RichText::new(&toast.text).color(theme::TEXT_STRONG));
                    });
                });
        });
    ctx.request_repaint();
}
