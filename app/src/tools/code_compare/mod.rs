//! Code Compare: two texts or files side by side (or as one unified list),
//! with line numbers, syntax highlighting and the changed words of each
//! modified line marked. Long unchanged stretches are folded away.
//!
//! This module draws the tool; [`diff`] computes the comparison and
//! [`language`] picks the syntax highlighting.
mod diff;
mod language;

use std::collections::HashSet;
use std::ops::Range;
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon, theme};
use diff::{change_starts, compare, fold, Cell, Comparison, Item, Kind, Options, Row};
use language::{language_label, language_of_file, sniff_language, LANGUAGES};

/// Longer lines are drawn without syntax highlighting.
const MAX_HIGHLIGHT_LINE: usize = 2_000;
/// Width of the gutter's +/− column, in points.
const SIGN_WIDTH: f32 = 18.0;

/// The example's original text: a small JavaScript module.
const SAMPLE_ORIGINAL: &str = "\
import { fetchUser } from './api';

export async function greet(id) {
  const user = await fetchUser(id);
  if (!user) {
    return 'Hello, stranger!';
  }
  return 'Hello, ' + user.name + '!';
}

export const VERSION = '1.0.0';
";
/// The example's modified text: the same module with a few lines changed and one added.
const SAMPLE_MODIFIED: &str = "\
import { fetchUser, logVisit } from './api';

export async function greet(id, { formal = false } = {}) {
  const user = await fetchUser(id);
  if (!user) {
    return formal ? 'Good day.' : 'Hello, stranger!';
  }
  logVisit(user.id);
  return `Hello, ${user.name}!`;
}

export const VERSION = '1.1.0';
";

/// How the differences are listed: two columns, or one column in patch order.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum View { #[default] SideBySide, Unified }

/// Gives the parts of `job` inside `ranges` a background, splitting sections where needed.
fn mark_words(job: &mut egui::text::LayoutJob, ranges: &[Range<usize>], color: egui::Color32) {
    let mut sections = Vec::with_capacity(job.sections.len() + 2 * ranges.len());
    for section in job.sections.drain(..) {
        // Cut the section at every range boundary inside it; the parts within a range get the background.
        let (start, end) = (section.byte_range.start, section.byte_range.end);
        let mut cuts = vec![start, end];
        cuts.extend(ranges.iter().flat_map(|r| [r.start, r.end]).filter(|&b| b > start && b < end));
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let mut part = section.clone();
            part.byte_range = pair[0]..pair[1];
            // Only the first part keeps the section's leading space.
            if pair[0] != start { part.leading_space = 0.0; }
            if ranges.iter().any(|r| r.start <= pair[0] && pair[1] <= r.end) { part.format.background = color; }
            sections.push(part);
        }
    }
    job.sections = sections;
}

/// How a line is drawn: its background, sign and changed-word color.
#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Plain,
    Removed,
    Added,
    /// The empty side facing an added or removed line.
    Filler,
}

/// What every line of a column needs to draw itself.
struct LineStyle<'a> {
    /// Language for syntect (name or extension); empty for plain text.
    language: &'a str,
    font: egui::FontId,
    /// Width of one line-number column, in points.
    number_width: f32,
    /// Width of the whole gutter (line numbers and sign), in points.
    gutter: f32,
}

/// Draws one line: background, highlighted text with its changed words, and a
/// gutter (line numbers and +/− sign) that stays in view when scrolling sideways.
fn paint_line(ui: &egui::Ui, rect: egui::Rect, style: &LineStyle, numbers: &[Option<usize>], tone: Tone, text: Option<(&str, &[Range<usize>])>) {
    let (row_bg, word_bg, sign, sign_color) = match tone {
        Tone::Removed => (theme::DIFF_REMOVED.gamma_multiply(0.13), theme::DIFF_REMOVED.gamma_multiply(0.4), "−", theme::DIFF_REMOVED),
        Tone::Added => (theme::DIFF_ADDED.gamma_multiply(0.11), theme::DIFF_ADDED.gamma_multiply(0.34), "+", theme::DIFF_ADDED),
        Tone::Filler => (theme::FILLER, egui::Color32::TRANSPARENT, "", egui::Color32::TRANSPARENT),
        Tone::Plain => (egui::Color32::TRANSPARENT, egui::Color32::TRANSPARENT, "", egui::Color32::TRANSPARENT),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, row_bg);
    if let Some((text, words)) = text {
        let mut job = if style.language.is_empty() || text.len() > MAX_HIGHLIGHT_LINE {
            egui::text::LayoutJob::simple_singleline(text.to_owned(), style.font.clone(), theme::TEXT)
        } else {
            ui::syntax_job(ui, text, style.language)
        };
        if !words.is_empty() { mark_words(&mut job, words, word_bg); }
        let galley = ui.fonts(|fonts| fonts.layout_job(job));
        let pos = egui::pos2(rect.left() + style.gutter + 8.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(pos, galley, theme::TEXT);
    }
    // The gutter sticks to the visible left edge, drawn over the text: opaque
    // to hide the text scrolled under it, then tinted like the row.
    let left = ui.clip_rect().left().max(rect.left());
    let gutter = egui::Rect::from_min_size(egui::pos2(left, rect.top()), egui::vec2(style.gutter, rect.height()));
    painter.rect_filled(gutter, 0.0, theme::GUTTER);
    painter.rect_filled(gutter, 0.0, row_bg);
    for (i, number) in numbers.iter().enumerate() {
        let Some(number) = number else { continue };
        let at = egui::pos2(gutter.left() + (i + 1) as f32 * style.number_width - 2.0, rect.center().y);
        painter.text(at, egui::Align2::RIGHT_CENTER, number.to_string(), style.font.clone(), theme::SUBTLE);
    }
    painter.text(egui::pos2(gutter.right() - SIGN_WIDTH / 2.0, rect.center().y), egui::Align2::CENTER_CENTER, sign, style.font.clone(), sign_color);
}

/// Draws a folded run of unchanged lines; returns true when it is clicked (to unfold it).
fn paint_fold(ui: &egui::Ui, rect: egui::Rect, response: &egui::Response, hidden: usize) -> bool {
    let hovered = response.hovered();
    ui.painter().rect_filled(rect, 0.0, if hovered { theme::ROW_SELECTED } else { theme::FOLD });
    let left = ui.clip_rect().left().max(rect.left()) + 14.0;
    let label = format!("{}   {hidden} unchanged lines", icon::ARROWS_OUT_LINE_VERTICAL);
    ui.painter().text(egui::pos2(left, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0),
        if hovered { theme::ACCENT } else { theme::MUTED });
    if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    response.clicked()
}

/// The text of a cell's line and its changed words.
fn cell_text<'a>(cell: &'a Option<Cell>, lines: &'a [String]) -> Option<(&'a str, &'a [Range<usize>])> {
    cell.as_ref().map(|c| (lines[c.line].as_str(), c.words.as_slice()))
}

/// The 1-based line number of a cell.
fn cell_number(cell: &Option<Cell>) -> Option<usize> { cell.as_ref().map(|c| c.line + 1) }

/// Height of one line of the difference view: a monospace line plus a little air.
fn row_height(ui: &egui::Ui) -> f32 { ui.text_style_height(&egui::TextStyle::Monospace) + 5.0 }

/// The Code Compare tool: two editors above the differences between them.
pub struct CodeCompare {
    /// The original (0) and modified (1) texts.
    texts: [String; 2],
    /// File names, when the texts were opened from files.
    names: [String; 2],
    error: String,
    view: View,
    /// Index into [`LANGUAGES`]; `None` detects the language.
    language: Option<usize>,
    /// The language found from the file names or the texts, for "Auto".
    detected: &'static str,
    options: Options,
    /// Whether long unchanged runs are folded.
    collapse: bool,
    /// Whether the two editors are shown (hidden to see more lines).
    show_texts: bool,
    result: Comparison,
    /// The lines drawn: the rows of the current view, with the folds.
    items: Vec<Item>,
    /// Folds the user opened, by the index of their first row.
    expanded: HashSet<usize>,
    /// Vertical scroll position, shared by both columns.
    scroll_y: f32,
    /// Compares again once typing pauses.
    live: ui::Debounce,
}

impl Default for CodeCompare {
    fn default() -> Self {
        Self {
            texts: Default::default(), names: Default::default(), error: String::new(), view: View::default(), language: None,
            detected: "", options: Options::default(), collapse: true, show_texts: true, result: Comparison::default(),
            items: Vec::new(), expanded: HashSet::new(), scroll_y: 0.0, live: ui::Debounce::default(),
        }
    }
}

impl CodeCompare {
    /// Compares the texts again and detects their language; opened folds close again.
    fn recompare(&mut self) {
        self.result = compare(&self.texts[0], &self.texts[1], &self.names, self.options);
        // From a file name if there is one, else sniffed from the modified text, then the original.
        self.detected = self.names.iter().find_map(|name| language_of_file(name)).unwrap_or_else(|| {
            let language = sniff_language(&self.texts[1]);
            if language.is_empty() { sniff_language(&self.texts[0]) } else { language }
        });
        self.expanded.clear();
        self.relayout();
    }

    /// The rows of the current view.
    fn rows(&self) -> &[Row] {
        match self.view { View::SideBySide => &self.result.rows, View::Unified => &self.result.unified }
    }

    /// Rebuilds the visible lines after the view, folding or the comparison changed.
    fn relayout(&mut self) {
        self.items = fold(self.rows(), self.collapse, &self.expanded);
    }

    /// The highlighting language: the chosen one, or the detected one for "Auto".
    fn language(&self) -> &'static str { self.language.map_or(self.detected, |i| LANGUAGES[i].1) }

    /// Loads files into the sides from `side` on (two files fill both sides).
    fn load_files(&mut self, side: usize, files: Vec<Result<(String, Vec<u8>), String>>) {
        let mut error = None;
        for (side, file) in (side..2).zip(files) {
            match file.and_then(|(name, bytes)| String::from_utf8(bytes).map(|text| (name.clone(), text)).map_err(|_| format!("{name} is not a UTF-8 text file"))) {
                Ok((name, text)) => { self.texts[side] = text; self.names[side] = name; }
                Err(e) => error = Some(e),
            }
        }
        // Set once, so that a file that opened fine does not hide another one's error.
        self.error = error.unwrap_or_default();
        self.recompare();
    }

    /// Files dropped on the window: two fill both sides; one goes to the
    /// original side when it is empty, the modified side otherwise.
    fn take_dropped(&mut self, ctx: &egui::Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        if files.is_empty() { return; }
        let side = if files.len() >= 2 || self.texts[0].is_empty() { 0 } else { 1 };
        self.load_files(side, files.iter().map(ui::read_dropped).collect());
    }

    /// The Original or Modified editor with its toolbar; returns true when the text changed.
    fn text_pane(&mut self, ui: &mut egui::Ui, side: usize) -> bool {
        let mut changed = false;
        let label = ["Original:", "Modified:"][side];
        ui::toolbar(ui, label, |ui| {
            if ui::icon_only(ui, icon::FOLDER_OPEN, "Open a file (pick two to fill both sides)") {
                let paths = ui::pick_files("Open a file to compare", &[]);
                if !paths.is_empty() { self.load_files(side, paths.iter().map(|p| ui::read_path(p)).collect()); }
            }
            if ui::paste_button(ui, &mut self.texts[side], &mut self.error) { self.names[side].clear(); changed = true; }
            if ui::clear_button(ui) { self.texts[side].clear(); self.names[side].clear(); changed = true; }
            if !self.names[side].is_empty() {
                ui.label(egui::RichText::new(format!("{}  {}", icon::FILE_TEXT, self.names[side])).color(theme::MUTED));
            }
        });
        let language = self.language();
        let highlight = ui::Highlight::syntax(Some(language).filter(|l| !l.is_empty()));
        let hint = ["Original text, or drop a file…", "Modified text, or drop a file…"][side];
        changed |= ui::code_editor(ui, ["original", "modified"][side], &mut self.texts[side], hint, highlight).changed();
        changed
    }

    /// The Differences toolbar: counts, view, language, options, navigation and actions.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let row_height = row_height(ui);
        let starts = change_starts(&self.items, self.rows());
        let (mut recompare, mut relayout) = (false, false);
        ui::toolbar(ui, "Differences:", |ui| {
            let r = &self.result;
            if r.added + r.removed > 0 {
                ui.label(egui::RichText::new(format!("+{}", r.added)).strong().color(theme::DIFF_ADDED));
                ui.label(egui::RichText::new(format!("−{}", r.removed)).strong().color(theme::DIFF_REMOVED));
                let n = starts.len();
                ui.label(egui::RichText::new(format!("in {n} {}", if n == 1 { "place" } else { "places" })).color(theme::MUTED));
            } else if self.texts.iter().any(|t| !t.is_empty()) {
                ui::badge(ui, icon::CHECK_CIRCLE, "No differences", theme::ACCENT);
            }
            ui.separator();
            let (side, unified) = (format!("{}  Side by side", icon::COLUMNS), format!("{}  Unified", icon::ROWS));
            if ui::segmented(ui, &mut self.view, &[(View::SideBySide, side.as_str()), (View::Unified, unified.as_str())]) {
                // Opened folds are row positions of the previous view.
                self.expanded.clear();
                relayout = true;
            }
            let auto = format!("Auto · {}", language_label(self.detected));
            let selected = self.language.map_or(auto.clone(), |i| LANGUAGES[i].0.to_owned());
            egui::ComboBox::from_id_salt("compare-language").selected_text(selected).width(150.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.language, None, auto);
                for (i, (label, _)) in LANGUAGES.iter().enumerate() { ui.selectable_value(&mut self.language, Some(i), *label); }
            }).response.on_hover_text("Syntax highlighting");
            ui.menu_button(format!("{}  Options", icon::SLIDERS_HORIZONTAL), |ui| {
                recompare |= ui.checkbox(&mut self.options.ignore_whitespace, "Ignore whitespace").changed();
                recompare |= ui.checkbox(&mut self.options.ignore_case, "Ignore case").changed();
                relayout |= ui.checkbox(&mut self.collapse, "Fold unchanged lines").changed();
            });
            ui.separator();
            // Jump to a change, with two lines of context above it.
            let target = |item: usize| (item as f32 - 2.0).max(0.0) * row_height;
            if ui::icon_only(ui, icon::CARET_UP, "Previous change") {
                if let Some(y) = starts.iter().rev().map(|&s| target(s)).find(|&y| y < self.scroll_y - 1.0) { self.scroll_y = y; }
            }
            if ui::icon_only(ui, icon::CARET_DOWN, "Next change") {
                if let Some(y) = starts.iter().map(|&s| target(s)).find(|&y| y > self.scroll_y + 1.0) { self.scroll_y = y; }
            }
            if ui::icon_only(ui, icon::ARROWS_LEFT_RIGHT, "Swap original and modified") {
                self.texts.swap(0, 1);
                self.names.swap(0, 1);
                recompare = true;
            }
            let (glyph, tip) = if self.show_texts { (icon::ARROWS_IN_LINE_VERTICAL, "Hide the texts to see more lines") } else { (icon::ARROWS_OUT_LINE_VERTICAL, "Show the texts") };
            if ui::icon_only(ui, glyph, tip) { self.show_texts = !self.show_texts; }
            if ui::sample_button(ui) { self.load_sample(); }
            if ui::clear_button(ui) {
                *self = Self { view: self.view, language: self.language, options: self.options, collapse: self.collapse, ..Self::default() };
            }
            ui::copy_button(ui, &self.result.patch);
        });
        if recompare { self.recompare(); } else if relayout { self.relayout(); }
    }

    /// One scrolling column of lines: a side of the side-by-side view
    /// (`side` = 0 or 1) or the unified view (`None`). Returns its scroll
    /// position; a clicked fold is reported in `clicked`.
    fn column(&self, ui: &mut egui::Ui, side: Option<usize>, clicked: &mut Option<usize>) -> f32 {
        let rows = self.rows();
        let r = &self.result;
        let font = egui::TextStyle::Monospace.resolve(ui.style());
        let char_width = ui.fonts(|fonts| fonts.glyph_width(&font, '0'));
        let digits = r.old.len().max(r.new.len()).max(1).to_string().len();
        let number_width = digits as f32 * char_width + 10.0;
        // One line-number column per side; the unified view shows both.
        let numbers = if side.is_some() { 1.0 } else { 2.0 };
        let gutter = numbers * number_width + SIGN_WIDTH;
        let style = LineStyle { language: self.language(), font, number_width, gutter };
        let widest = side.map_or(r.widest[0].max(r.widest[1]), |s| r.widest[s]);
        let content_width = gutter + widest as f32 * char_width + 32.0;
        let row_height = row_height(ui);
        ui::editor_frame().inner_margin(1).show(ui, |ui| {
            let width = ui.available_width();
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            // Only the visible rows are laid out, so long texts stay fast. The offset
            // is set every frame to keep the two columns in step.
            egui::ScrollArea::both().id_salt(("compare", side)).auto_shrink([false, false]).vertical_scroll_offset(self.scroll_y)
                .show_rows(ui, row_height, self.items.len(), |ui, range| {
                    let row_width = width.max(content_width);
                    ui.set_min_width(row_width);
                    for item in &self.items[range] {
                        let sense = if matches!(item, Item::Fold { .. }) { egui::Sense::click() } else { egui::Sense::hover() };
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(row_width, row_height), sense);
                        let row = match *item {
                            Item::Fold { start, end } => {
                                if paint_fold(ui, rect, &response, end - start) { *clicked = Some(start); }
                                continue;
                            }
                            Item::Row(i) => &rows[i],
                        };
                        let (text, number) = (cell_text, cell_number);
                        match side {
                            Some(0) => {
                                let tone = match row.kind { Kind::Equal => Tone::Plain, Kind::Added => Tone::Filler, _ => Tone::Removed };
                                paint_line(ui, rect, &style, &[number(&row.left)], tone, text(&row.left, &r.old));
                            }
                            Some(_) => {
                                let tone = match row.kind { Kind::Equal => Tone::Plain, Kind::Removed => Tone::Filler, _ => Tone::Added };
                                paint_line(ui, rect, &style, &[number(&row.right)], tone, text(&row.right, &r.new));
                            }
                            None => {
                                let (tone, shown) = match row.kind {
                                    Kind::Added => (Tone::Added, text(&row.right, &r.new)),
                                    Kind::Removed => (Tone::Removed, text(&row.left, &r.old)),
                                    _ => (Tone::Plain, text(&row.left, &r.old)),
                                };
                                paint_line(ui, rect, &style, &[number(&row.left), number(&row.right)], tone, shown);
                            }
                        }
                    }
                }).state.offset.y
        }).inner
    }

    /// The difference view, or a hint while both texts are empty.
    fn differences(&mut self, ui: &mut egui::Ui) {
        if self.texts.iter().all(String::is_empty) {
            ui::preview_frame(ui, |ui| {
                ui.add_space(ui.available_height() / 2.0 - 30.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(icon::GIT_DIFF).size(30.0).color(theme::SUBTLE));
                    ui.label(egui::RichText::new("Type, paste or open two texts — or drop two files on the window.").color(theme::SUBTLE));
                });
            });
            return;
        }
        let mut clicked = None;
        match self.view {
            View::SideBySide => {
                let offsets = ui::split(ui, |left, right| [self.column(left, Some(0), &mut clicked), self.column(right, Some(1), &mut clicked)]);
                // Both columns scroll together: follow whichever one the user scrolled.
                if let Some(y) = offsets.into_iter().find(|y| (y - self.scroll_y).abs() > 0.5) { self.scroll_y = y; }
            }
            View::Unified => self.scroll_y = self.column(ui, None, &mut clicked),
        }
        if let Some(start) = clicked {
            self.expanded.insert(start);
            self.relayout();
        }
    }
}

impl Tool for CodeCompare {
    fn name(&self) -> &'static str { "Code Compare" }
    fn icon(&self) -> &'static str { icon::GIT_DIFF }
    fn category(&self) -> Category { Category::Compare }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let area = ui.available_rect_before_wrap();
        self.take_dropped(&ctx);
        ui::description(ui, "Compare two texts or files, with changed words highlighted. Copy gives a unified patch.");
        ui.add_space(10.0);
        if self.show_texts {
            let height = (ui.available_height() * 0.34).max(120.0);
            let changed = ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| {
                ui::split(ui, |left, right| self.text_pane(left, 0) | self.text_pane(right, 1))
            }).inner;
            if changed { self.live.touch(&ctx); }
            ui.add_space(8.0);
        }
        // Compare as you type (after a short pause for large texts).
        if self.live.due(&ctx, self.texts[0].len() + self.texts[1].len()) { self.recompare(); }
        self.toolbar(ui);
        ui::error_label(ui, &self.error);
        self.differences(ui);
        ui::drop_hint(&ctx, area, "Drop one or two files to compare");
    }

    fn load_sample(&mut self) {
        self.texts = [SAMPLE_ORIGINAL.into(), SAMPLE_MODIFIED.into()];
        self.names = Default::default();
        self.recompare();
    }

    fn accepts_input(&self) -> bool { true }

    /// Received text becomes the "Modified" side.
    fn set_input(&mut self, text: &str) {
        self.texts[1] = text.to_owned();
        self.names[1].clear();
        self.recompare();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_words_inside_highlighted_sections() {
        let mut job = egui::text::LayoutJob::simple_singleline("let b = 2;".into(), egui::FontId::monospace(12.0), theme::TEXT);
        mark_words(&mut job, std::slice::from_ref(&(8..10)), theme::MATCH_BG);
        let parts: Vec<(Range<usize>, bool)> = job.sections.iter().map(|s| (s.byte_range.clone(), s.format.background == theme::MATCH_BG)).collect();
        assert_eq!(parts, [(0..8, false), (8..10, true)]);
    }
}
