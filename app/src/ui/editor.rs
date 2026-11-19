//! Text editors and read-only previews, with syntax highlighting and the
//! other highlight styles (ranges, colored spans).
use eframe::egui;
use super::{theme, MUTED};

/// Texts longer than this are shown without syntax highlighting, which would
/// make typing sluggish on very large inputs.
const MAX_HIGHLIGHT_BYTES: usize = 200_000;

/// The dark rounded box around editors and previews.
pub fn editor_frame() -> egui::Frame {
    egui::Frame::new().fill(theme::EDITOR_BG)
        .stroke(egui::Stroke::new(1.0_f32, theme::EDITOR_STROKE))
        .corner_radius(6).inner_margin(8)
}

/// How an editor colors its text.
pub enum Highlight<'a> {
    /// All text in [`theme::TEXT`], with no layouter (the fastest).
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
            // Each layouter closure has its own type, hence one `ui.add` per kind.
            match highlight {
                Highlight::Plain => ui.add(editor.text_color(theme::TEXT)),
                Highlight::Syntax(language) => {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| layout(ui, syntax_job(ui, string, language), wrap_width);
                    ui.add(editor.layouter(&mut layouter))
                }
                Highlight::Ranges(ranges) => {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| layout(ui, ranges_job(ui, string, ranges), wrap_width);
                    ui.add(editor.layouter(&mut layouter))
                }
                Highlight::Spans(spans) => {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| layout(ui, spans_job(ui, string, spans), wrap_width);
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

/// Wraps `job` at `wrap_width` and lays it out, as a `TextEdit` layouter must.
fn layout(ui: &egui::Ui, mut job: egui::text::LayoutJob, wrap_width: f32) -> std::sync::Arc<egui::Galley> {
    job.wrap.max_width = wrap_width;
    ui.fonts(|fonts| fonts.layout_job(job))
}

/// Syntax-highlighted layout of `text` (results are cached by `egui_extras`).
pub fn syntax_job(ui: &egui::Ui, text: &str, language: &str) -> egui::text::LayoutJob {
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
    let mut job = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, text, language);
    // egui_extras 0.31 underlines everything italic; keep the italics only.
    for section in &mut job.sections { section.format.underline = egui::Stroke::NONE; }
    job
}

/// Read-only, selectable, highlighted code sized to its content (indentation is kept).
pub fn code_view(ui: &mut egui::Ui, code: &str, language: &str) {
    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| layout(ui, syntax_job(ui, string, language), wrap_width);
    // A `&str` buffer makes the `TextEdit` read-only but still selectable.
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
        let Some(part) = fitting(text, range, pos) else { continue };
        job.append(&text[pos..range.start], 0.0, normal.clone());
        job.append(part, 0.0, marked.clone());
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
        let Some(part) = fitting(text, range, pos) else { continue };
        job.append(&text[pos..range.start], 0.0, egui::TextFormat::simple(font.clone(), MUTED));
        job.append(part, 0.0, egui::TextFormat::simple(font.clone(), *color));
        pos = range.end;
    }
    job.append(&text[pos..], 0.0, egui::TextFormat::simple(font, MUTED));
    job
}

/// The part of `text` in `range`, if the range is valid for it (in bounds, on
/// char boundaries) and starts at or after byte `pos` (ranges must be sorted).
fn fitting<'t>(text: &'t str, range: &std::ops::Range<usize>, pos: usize) -> Option<&'t str> {
    text.get(range.clone()).filter(|_| range.start >= pos)
}

/// `word`, with an "s" unless `n` is 1.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `check` with a `Ui` from a headless frame.
    fn with_ui(check: impl FnOnce(&mut egui::Ui)) {
        let ctx = egui::Context::default();
        let mut check = Some(check);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| if let Some(check) = check.take() { check(ui) });
        });
    }

    /// The parts of `job` drawn with `background`.
    fn parts_on(job: &egui::text::LayoutJob, background: egui::Color32) -> Vec<String> {
        job.sections.iter().filter(|s| s.format.background == background).map(|s| job.text[s.byte_range.clone()].to_owned()).collect()
    }

    #[test]
    fn ranges_that_do_not_fit_are_skipped() {
        // "é" takes bytes 1..3.
        let text = "héllo world";
        assert_eq!(fitting(text, &(0..1), 0), Some("h"));
        assert_eq!(fitting(text, &(1..2), 0), None, "inside a character");
        assert_eq!(fitting(text, &(3..50), 0), None, "past the end");
        assert_eq!(fitting(text, &(0..1), 2), None, "before the previous range");
        with_ui(|ui| {
            // Stale ranges (the text changed since they were found) never lose text or panic.
            let job = ranges_job(ui, text, &[0..1, 1..2, 7..12, 3..5, 40..41]);
            assert_eq!(job.text, text);
            assert_eq!(parts_on(&job, theme::MATCH_BG), ["h", "world"]);
        });
    }

    #[test]
    fn spans_color_their_part_and_mute_the_rest() {
        with_ui(|ui| {
            let job = spans_job(ui, "aaa.bbb.ccc", &[(0..3, theme::ROSE), (4..7, theme::VIOLET), (6..20, theme::CYAN)]);
            assert_eq!(job.text, "aaa.bbb.ccc");
            let colored: Vec<(String, egui::Color32)> = job.sections.iter().map(|s| (job.text[s.byte_range.clone()].to_owned(), s.format.color)).filter(|(t, _)| !t.is_empty()).collect();
            assert_eq!(colored, [("aaa".to_owned(), theme::ROSE), (".".to_owned(), MUTED), ("bbb".to_owned(), theme::VIOLET), (".ccc".to_owned(), MUTED)]);
        });
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(plural(1, "line"), "line");
        assert_eq!(plural(0, "char"), "chars");
        assert_eq!(plural(2, "line"), "lines");
    }
}
