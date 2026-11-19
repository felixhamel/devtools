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
    pub const LIME: Color32 = Color32::from_rgb(185, 225, 110);
}

pub use theme::MUTED;

/// Output "language" of `Key: value` reports, shown with [`properties`].
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
    // Single-line fields share the editors' background instead of black.
    visuals.extreme_bg_color = theme::EDITOR_BG;
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
    copy_with_feedback(ui, text, ui.id().with("copy-feedback"), CopyStyle::Labeled);
}

/// Icon-only copy button (a check mark appears after a click). `id` must be
/// unique among the buttons shown together, e.g. the row index in a table.
pub fn copy_icon(ui: &mut egui::Ui, text: &str, id: impl std::hash::Hash) {
    copy_with_feedback(ui, text, egui::Id::new(("copy-icon", id)), CopyStyle::Icon);
}

/// How a copy button looks: "Copy" with an icon, the icon alone, or a faint
/// icon (in lists, until the row is hovered).
#[derive(Clone, Copy, PartialEq)]
enum CopyStyle { Labeled, Icon, Faint }

fn copy_with_feedback(ui: &mut egui::Ui, text: &str, id: egui::Id, style: CopyStyle) {
    const FEEDBACK: f64 = 1.5;
    let now = ui.input(|i| i.time);
    // Seconds of "Copied" feedback left, if the button was clicked recently.
    let remaining = ui.data(|d| d.get_temp::<f64>(id)).map(|at| FEEDBACK - (now - at)).filter(|left| *left > 0.0);
    let (glyph, label) = if remaining.is_some() { (icon::CHECK, "Copied") } else { (icon::COPY, "Copy") };
    let button = match style {
        CopyStyle::Labeled => icon_button(glyph, label, false),
        CopyStyle::Icon | CopyStyle::Faint => {
            let faint = style == CopyStyle::Faint && remaining.is_none();
            let color = if remaining.is_some() { theme::ACCENT } else if faint { theme::SUBTLE } else { theme::TEXT };
            let fill = if faint { egui::Color32::TRANSPARENT } else { theme::BUTTON };
            egui::Button::new(egui::RichText::new(glyph).size(13.0).color(color)).fill(fill).corner_radius(5)
        }
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
    /// Byte ranges drawn in the given text colors (e.g. the three parts of a JWT).
    Spans(&'a [(std::ops::Range<usize>, egui::Color32)]),
}

impl<'a> Highlight<'a> {
    /// Highlighting for an optional language name.
    pub fn syntax(language: Option<&'a str>) -> Self {
        language.map_or(Highlight::Plain, Highlight::Syntax)
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

fn plural(n: usize, word: &str) -> String { if n == 1 { word.to_owned() } else { format!("{word}s") } }

/// A read-only area that fills the remaining space, for previews drawn with widgets.
pub fn preview_frame(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    editor_frame().show(ui, |ui| {
        let size = ui.available_size();
        // Side-by-side columns share their base id: salt with the frame's unique id
        // so two previews never share a scroll position (or clash).
        egui::ScrollArea::vertical().id_salt(ui.next_auto_id()).auto_shrink([false, false]).show(ui, |ui| {
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

    /// Draws the input pane (toolbar + editor). With several actions, a
    /// segmented control picks the mode, which re-runs live as the input
    /// changes. Returns true when the result should be recomputed now
    /// (mode changed, sample loaded, or the input changed).
    pub fn input_pane(&mut self, ui: &mut egui::Ui, actions: &[&str], hint: &str, sample: Option<&str>) -> bool {
        self.input_pane_with(ui, actions, hint, sample, |_| false)
    }

    /// Like [`TextIo::input_pane`], with `extra` controls at the end of the
    /// toolbar (e.g. options of the mode); `extra` returns true to re-run.
    pub fn input_pane_with(&mut self, ui: &mut egui::Ui, actions: &[&str], hint: &str, sample: Option<&str>, extra: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
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
            if actions.len() > 1 {
                ui.separator();
                let options: Vec<(usize, &str)> = actions.iter().copied().enumerate().collect();
                run_now |= segmented(ui, &mut self.active, &options);
            }
            run_now |= extra(ui);
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
        if self.output_syntax == Some(KEY_VALUE) {
            properties(ui, "output", &self.output);
        } else {
            code_editor(ui, "output", &mut self.output, "", Highlight::syntax(self.output_syntax));
        }
        result
    }
}

/// Joined toggle buttons of which exactly one is selected, e.g. view modes.
/// Returns true when the user picked another option.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, current: &mut T, options: &[(T, &str)]) -> bool {
    const SIZE: f32 = 12.5;
    // A frame cannot wrap by itself: in a wrapping toolbar, start a new line when it does not fit.
    if ui.layout().main_wrap {
        let padding = 2.0 * ui.spacing().button_padding.x + 2.0;
        let width: f32 = options.iter()
            .map(|(_, label)| ui.fonts(|f| f.layout_no_wrap((*label).to_owned(), egui::FontId::proportional(SIZE), theme::TEXT).size().x) + padding)
            .sum::<f32>() + 6.0;
        if width > ui.available_size_before_wrap().x { ui.end_row(); }
    }
    let mut changed = false;
    egui::Frame::new().fill(theme::SEARCH_BG).stroke(egui::Stroke::new(1.0_f32, theme::EDITOR_STROKE))
        .corner_radius(6).inner_margin(1)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            // As tall as the toolbar's other buttons.
            ui.spacing_mut().button_padding.y = 1.0;
            ui.horizontal(|ui| {
                for (value, label) in options {
                    let selected = *current == *value;
                    let text = egui::RichText::new(*label).size(SIZE).color(if selected { theme::ON_ACCENT } else { theme::TEXT });
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

/// A line of a `Key: value` report (see [`parse_properties`]).
#[derive(Debug, PartialEq)]
pub enum Property<'a> {
    /// `Key: value`; `indent` marks lines that belong to the heading above.
    Pair { key: &'a str, value: &'a str, indent: bool },
    /// A line without a key, e.g. an item listed under a heading.
    Text { value: &'a str, indent: bool },
    /// A `Heading:` line followed by indented lines.
    Heading(&'a str),
    /// A blank line between groups.
    Gap,
}

/// Splits a report into [`Property`] lines: `Key: value` (split at the first
/// ": "), `Heading:` followed by indented lines (a single indented line
/// becomes the heading's value), other lines, and blank lines.
pub fn parse_properties(text: &str) -> Vec<Property<'_>> {
    let lines: Vec<&str> = text.lines().collect();
    let is_indented = |line: &str| line.starts_with(char::is_whitespace) && !line.trim().is_empty();
    let mut items = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        let indent = is_indented(line);
        let trimmed = line.trim_start();
        i += 1;
        if trimmed.is_empty() {
            items.push(Property::Gap);
        } else if let Some((key, value)) = trimmed.split_once(": ") {
            items.push(Property::Pair { key, value: value.trim(), indent });
        } else if let (Some(key), false) = (trimmed.strip_suffix(':'), indent) {
            let block = lines[i..].iter().take_while(|l| is_indented(l)).count();
            if block == 1 {
                items.push(Property::Pair { key, value: lines[i].trim(), indent: false });
                i += 1;
            } else {
                items.push(Property::Heading(key));
            }
        } else {
            items.push(Property::Text { value: trimmed, indent });
        }
    }
    items
}

/// Shows a `Key: value` report as rows: muted keys, bright selectable values
/// and a copy button for each value. `id` must be unique within the tool.
pub fn properties(ui: &mut egui::Ui, id: &str, text: &str) {
    property_list(ui, id, &parse_properties(text));
}

/// Draws [`Property`] rows in a scrolling frame, as [`properties`] does.
pub fn property_list(ui: &mut egui::Ui, id: &str, items: &[Property]) {
    const INDENT: f32 = 16.0;
    const COPY_WIDTH: f32 = 32.0;
    let key_font = egui::FontId::proportional(13.0);
    let widest = items.iter()
        .filter_map(|item| match item { Property::Pair { key, indent, .. } => Some((*key, *indent)), _ => None })
        .map(|(key, indent)| ui.fonts(|f| f.layout_no_wrap(key.to_owned(), key_font.clone(), theme::MUTED).size().x) + if indent { INDENT } else { 0.0 })
        .fold(0.0_f32, f32::max);
    // The hovered row (from the previous frame) shows its copy button fully.
    let hover_id = egui::Id::new((id, "hovered-row"));
    let hovered: Option<usize> = ui.data(|d| d.get_temp(hover_id)).unwrap_or(None);
    let mut now_hovered = None;
    preview_frame(ui, |ui| {
        let key_width = (widest + 14.0).min(ui.available_width() * 0.42);
        for (n, item) in items.iter().enumerate() {
            let (key, value, indent) = match *item {
                Property::Gap => { ui.add_space(8.0); continue; }
                Property::Heading(text) => {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(text).color(theme::MUTED));
                    continue;
                }
                Property::Pair { key, value, indent } => (Some(key), value, indent),
                Property::Text { value, indent } => (None, value, indent),
            };
            let background = ui.painter().add(egui::Shape::Noop);
            let row = ui.horizontal_top(|ui| {
                if indent { ui.add_space(INDENT); }
                if let Some(key) = key {
                    let width = (key_width - if indent { INDENT } else { 0.0 }).max(20.0);
                    ui.allocate_ui_with_layout(egui::vec2(width, 0.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.set_width(width);
                        ui.add(egui::Label::new(egui::RichText::new(key).font(key_font.clone()).color(theme::MUTED)).truncate());
                    });
                }
                let width = (ui.available_width() - COPY_WIDTH).max(40.0);
                ui.allocate_ui_with_layout(egui::vec2(width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(width);
                    ui.add(egui::Label::new(egui::RichText::new(value).monospace().color(theme::TEXT_STRONG)).wrap());
                });
                let style = if hovered == Some(n) { CopyStyle::Icon } else { CopyStyle::Faint };
                copy_with_feedback(ui, value, egui::Id::new((id, "copy", n)), style);
            });
            let rect = row.response.rect.expand2(egui::vec2(6.0, 2.0));
            if ui.rect_contains_pointer(rect) {
                now_hovered = Some(n);
                ui.painter().set(background, egui::epaint::RectShape::filled(rect, 5.0, theme::ROW_HOVER));
            }
        }
    });
    if now_hovered != hovered {
        ui.data_mut(|d| d.insert_temp(hover_id, now_hovered));
        ui.ctx().request_repaint();
    }
}

/// A byte count for people: "512 B", "2.4 KB", "3.1 MB".
pub fn human_size(bytes: usize) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

/// Space that centers content of `height` vertically in what is left of `ui`.
pub fn center_vertically(ui: &mut egui::Ui, height: f32) {
    ui.add_space(((ui.available_height() - height) / 2.0).max(0.0));
}

/// A centered icon and hint in an empty preview, e.g. before an image is loaded.
pub fn empty_state(ui: &mut egui::Ui, glyph: &str, text: &str) {
    ui.add_space((ui.available_height() / 2.0 - 40.0).max(8.0));
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(glyph).size(34.0).color(theme::SUBTLE));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(text).color(theme::SUBTLE));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_value_reports() {
        let report = "Read as: seconds\nUTC: Tue 14 Nov 2023, 22:13:20\n\nNext runs:\n  Mon 09:00\n  Tue 09:00\nSerial:\n  2D:F4\n2 matches\n  Group 1 (user): \"ada\"";
        assert_eq!(parse_properties(report), [
            Property::Pair { key: "Read as", value: "seconds", indent: false },
            Property::Pair { key: "UTC", value: "Tue 14 Nov 2023, 22:13:20", indent: false },
            Property::Gap,
            Property::Heading("Next runs"),
            Property::Text { value: "Mon 09:00", indent: true },
            Property::Text { value: "Tue 09:00", indent: true },
            Property::Pair { key: "Serial", value: "2D:F4", indent: false },
            Property::Text { value: "2 matches", indent: false },
            Property::Pair { key: "Group 1 (user)", value: "\"ada\"", indent: true },
        ]);
    }
}
