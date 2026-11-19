//! Code Compare: two texts or files side by side (or as one unified list),
//! with line numbers, syntax highlighting and the changed words of each
//! modified line marked. Long unchanged stretches are folded away.
use std::collections::HashSet;
use std::ops::Range;
use std::time::Duration;
use eframe::egui;
use similar::{ChangeTag, DiffOp};
use super::{Category, Tool};
use crate::ui::{self, icon, theme};

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

/// Unchanged lines kept visible around each change when folding.
const CONTEXT: usize = 3;
/// Shorter unchanged stretches are never folded.
const MIN_FOLD: usize = 4;
/// Longer lines are drawn without syntax highlighting.
const MAX_HIGHLIGHT_LINE: usize = 2_000;

/// Highlighting choices: (label, syntect language name or extension); "" is plain text.
const LANGUAGES: [(&str, &str); 18] = [
    ("Plain text", ""), ("JSON", "json"), ("JavaScript / TypeScript", "js"), ("Rust", "rs"), ("Python", "py"),
    ("Go", "go"), ("Java / Kotlin", "java"), ("C / C++", "cpp"), ("C#", "cs"), ("PHP", "php"), ("Ruby", "rb"),
    ("Shell", "sh"), ("SQL", "sql"), ("HTML", "html"), ("CSS", "css"), ("XML", "xml"), ("YAML", "yaml"), ("Markdown", "md"),
];

fn language_label(language: &str) -> &'static str {
    LANGUAGES.iter().find(|(_, l)| *l == language).map_or("Plain text", |(label, _)| label)
}

/// The language of a file name's extension, if it is one of [`LANGUAGES`].
fn language_of_file(name: &str) -> Option<&'static str> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "json" => "json",
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" => "js",
        "rs" => "rs",
        "py" => "py",
        "go" => "go",
        "java" | "kt" | "kts" => "java",
        "c" | "h" | "cc" | "cpp" | "hpp" | "m" | "mm" => "cpp",
        "cs" => "cs",
        "php" => "php",
        "rb" => "rb",
        "sh" | "bash" | "zsh" => "sh",
        "sql" => "sql",
        "html" | "htm" | "vue" | "svelte" => "html",
        "css" | "scss" | "less" => "css",
        "xml" | "svg" | "plist" => "xml",
        "yml" | "yaml" => "yaml",
        "md" | "markdown" => "md",
        _ => return None,
    })
}

/// Guesses the language of a text from a few unmistakable cues; plain text otherwise.
fn sniff_language(text: &str) -> &'static str {
    let start = text.trim_start();
    let first_word = start.split_whitespace().next().unwrap_or_default().to_ascii_uppercase();
    if (start.starts_with('{') || start.starts_with('[')) && serde_json::from_str::<serde_json::Value>(text).is_ok() { "json" }
    else if start.starts_with("<?xml") { "xml" }
    else if start.starts_with('<') { "html" }
    else if start.starts_with("#!") && start.lines().next().is_some_and(|line| line.contains("sh")) { "sh" }
    else if ["SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "WITH"].contains(&first_word.as_str()) { "sql" }
    else if text.contains("fn ") && (text.contains("let ") || text.contains("->")) { "rs" }
    else if text.contains("def ") && text.contains("):") { "py" }
    else if text.contains("package ") && text.contains("func ") { "go" }
    else if ["function ", "const ", "let ", "=>", "import ", "export "].iter().any(|cue| text.contains(cue)) { "js" }
    else { "" }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum View { #[default] SideBySide, Unified }

#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct Options { ignore_whitespace: bool, ignore_case: bool }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind { Equal, Removed, Added, Changed }

/// One side of a row: the line's index in its text and the byte ranges of its changed words.
#[derive(Clone, Debug, PartialEq)]
struct Cell { line: usize, words: Vec<Range<usize>> }

impl Cell {
    fn new(line: usize) -> Self { Cell { line, words: Vec::new() } }
}

/// A row of the view. `Changed` rows have both cells, `Removed` only the
/// left one and `Added` only the right one.
#[derive(Clone, Debug)]
struct Row { kind: Kind, left: Option<Cell>, right: Option<Cell> }

impl Row {
    fn equal(old: usize, new: usize) -> Self { Row { kind: Kind::Equal, left: Some(Cell::new(old)), right: Some(Cell::new(new)) } }
    fn removed(old: usize) -> Self { Row { kind: Kind::Removed, left: Some(Cell::new(old)), right: None } }
    fn added(new: usize) -> Self { Row { kind: Kind::Added, left: None, right: Some(Cell::new(new)) } }
}

/// A line of the view: a row, or a run of unchanged rows folded into one line.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item { Row(usize), Fold { start: usize, end: usize } }

#[derive(Default)]
struct Comparison {
    old: Vec<String>,
    new: Vec<String>,
    /// Side-by-side rows: a modified line has both cells.
    rows: Vec<Row>,
    /// The same changes as in a patch: removed lines, then the added ones.
    unified: Vec<Row>,
    added: usize,
    removed: usize,
    /// Longest line of each side, in characters (sets the horizontal scroll range).
    widest: [usize; 2],
    /// Standard unified diff, for the Copy button.
    patch: String,
}

/// Lines of `text`, with tabs expanded so that they line up when drawn.
fn split_lines(text: &str) -> Vec<String> {
    text.lines().map(|line| line.replace('\t', "    ")).collect()
}

/// Compares two texts line by line, then word by word within modified lines.
/// Gives up on the smallest diff after a short deadline, so huge inputs stay responsive.
fn compare(old_text: &str, new_text: &str, names: &[String; 2], options: Options) -> Comparison {
    let (old, new) = (split_lines(old_text), split_lines(new_text));
    let key = |line: &String| {
        let line = if options.ignore_whitespace { line.split_whitespace().collect() } else { line.clone() };
        if options.ignore_case { line.to_lowercase() } else { line }
    };
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    let ops = similar::capture_diff_slices_by_key_deadline(similar::Algorithm::Myers, &old, &new, key, Some(deadline));
    let mut rows = Vec::new();
    for op in ops {
        match op {
            DiffOp::Equal { old_index, new_index, len } => rows.extend((0..len).map(|i| Row::equal(old_index + i, new_index + i))),
            DiffOp::Delete { old_index, old_len, .. } => rows.extend((old_index..old_index + old_len).map(Row::removed)),
            DiffOp::Insert { new_index, new_len, .. } => rows.extend((new_index..new_index + new_len).map(Row::added)),
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                for pair in pair_lines(&old[old_index..old_index + old_len], &new[new_index..new_index + new_len]) {
                    rows.push(match pair {
                        (Some(o), Some(n)) => {
                            let (o, n) = (old_index + o, new_index + n);
                            let (left, right) = word_changes(&old[o], &new[n]);
                            Row { kind: Kind::Changed, left: Some(Cell { line: o, words: left }), right: Some(Cell { line: n, words: right }) }
                        }
                        (Some(o), None) => Row::removed(old_index + o),
                        (None, n) => Row::added(new_index + n.unwrap_or_default()),
                    });
                }
            }
        }
    }
    let changed = |row: &&Row| row.kind != Kind::Equal;
    let removed = rows.iter().filter(changed).filter(|row| row.left.is_some()).count();
    let added = rows.iter().filter(changed).filter(|row| row.right.is_some()).count();
    let widest = [&old, &new].map(|lines| lines.iter().map(|line| line.chars().count()).max().unwrap_or(0));
    let patch = if added + removed == 0 { String::new() } else {
        let header = |i: usize, fallback: &'static str| if names[i].is_empty() { fallback.to_owned() } else { names[i].clone() };
        similar::TextDiff::configure().timeout(Duration::from_millis(500)).diff_lines(old_text, new_text)
            .unified_diff().context_radius(CONTEXT).header(&header(0, "original"), &header(1, "modified")).to_string()
    };
    let unified = unify(&rows);
    Comparison { old, new, rows, unified, added, removed, widest, patch }
}

/// Lines that look alike enough to be shown side by side as one modified line.
const MIN_SIMILARITY: f32 = 0.45;
/// Larger blocks of changes are paired in order, without looking for similar lines.
const MAX_PAIRING_CELLS: usize = 2_500;

/// How alike two lines are, from 0 to 1: the share of character pairs
/// (bigrams) they have in common (Dice coefficient), ignoring indentation.
fn similarity(a: &str, b: &str) -> f32 {
    let bigrams = |s: &str| {
        let chars: Vec<char> = s.trim().chars().collect();
        let mut pairs: Vec<(char, char)> = chars.windows(2).map(|w| (w[0], w[1])).collect();
        pairs.sort_unstable();
        pairs
    };
    let (a, b) = (bigrams(a), bigrams(b));
    if a.is_empty() || b.is_empty() { return 0.0; }
    // Count common bigrams (with repeats) by walking both sorted lists.
    let (mut i, mut j, mut common) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => { common += 1; i += 1; j += 1; }
        }
    }
    2.0 * common as f32 / (a.len() + b.len()) as f32
}

/// Lines up a block of removed lines with the block that replaced it, in
/// order: `(Some(old), Some(new))` for a modified line, one `None` for a line
/// that stands alone. Similar lines are paired first (the most similar
/// overall, with a small alignment table); the lines between them are paired
/// in order.
fn pair_lines(old: &[String], new: &[String]) -> Vec<(Option<usize>, Option<usize>)> {
    let (n, m) = (old.len(), new.len());
    if n * m > MAX_PAIRING_CELLS {
        return (0..n.max(m)).map(|i| (Some(i).filter(|&i| i < n), Some(i).filter(|&i| i < m))).collect();
    }
    let sim: Vec<Vec<f32>> = old.iter().map(|o| new.iter().map(|w| similarity(o, w)).collect()).collect();
    // best[i][j]: highest total similarity pairing old[i..] with new[j..].
    let mut best = vec![vec![0.0_f32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let paired = if sim[i][j] >= MIN_SIMILARITY { sim[i][j] + best[i + 1][j + 1] } else { f32::MIN };
            best[i][j] = paired.max(best[i + 1][j]).max(best[i][j + 1]);
        }
    }
    // Walk the table; lines left between two similar pairs are zipped side by side.
    let mut pairs = Vec::with_capacity(n.max(m));
    let (mut gap_old, mut gap_new) = (Vec::new(), Vec::new());
    let flush = |pairs: &mut Vec<_>, gap_old: &mut Vec<usize>, gap_new: &mut Vec<usize>| {
        for k in 0..gap_old.len().max(gap_new.len()) { pairs.push((gap_old.get(k).copied(), gap_new.get(k).copied())); }
        gap_old.clear();
        gap_new.clear();
    };
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && sim[i][j] >= MIN_SIMILARITY && best[i][j] == sim[i][j] + best[i + 1][j + 1] {
            flush(&mut pairs, &mut gap_old, &mut gap_new);
            pairs.push((Some(i), Some(j)));
            i += 1;
            j += 1;
        } else if j < m && (i == n || best[i][j] == best[i][j + 1]) {
            gap_new.push(j);
            j += 1;
        } else {
            gap_old.push(i);
            i += 1;
        }
    }
    flush(&mut pairs, &mut gap_old, &mut gap_new);
    pairs
}

/// Byte ranges of the words that differ between two versions of a line.
fn word_changes(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let diff = similar::TextDiff::configure().timeout(Duration::from_millis(20)).diff_words(old, new);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut o, mut n) = (0, 0);
    for change in diff.iter_all_changes() {
        let len = change.value().len();
        match change.tag() {
            ChangeTag::Equal => { o += len; n += len; }
            ChangeTag::Delete => { push_merged(&mut left, o..o + len); o += len; }
            ChangeTag::Insert => { push_merged(&mut right, n..n + len); n += len; }
        }
    }
    (left, right)
}

/// Appends `range`, merging it with the previous one when they touch.
fn push_merged(ranges: &mut Vec<Range<usize>>, range: Range<usize>) {
    match ranges.last_mut() {
        Some(last) if last.end == range.start => last.end = range.end,
        _ => ranges.push(range),
    }
}

/// Side-by-side rows as a patch reads: within each block of changes, the
/// removed lines first, then the added ones.
fn unify(rows: &[Row]) -> Vec<Row> {
    let mut out = Vec::with_capacity(rows.len());
    let mut added = Vec::new();
    for row in rows {
        if row.kind == Kind::Equal {
            out.append(&mut added);
            out.push(row.clone());
            continue;
        }
        if let Some(cell) = &row.left { out.push(Row { kind: Kind::Removed, left: Some(cell.clone()), right: None }); }
        if let Some(cell) = &row.right { added.push(Row { kind: Kind::Added, left: None, right: Some(cell.clone()) }); }
    }
    out.append(&mut added);
    out
}

/// The lines to show: every row, or (when `collapse` is on) each long run of
/// unchanged rows folded down to [`CONTEXT`] lines around the changes,
/// except the folds the user opened (`expanded`, by first row).
fn fold(rows: &[Row], collapse: bool, expanded: &HashSet<usize>) -> Vec<Item> {
    let mut items = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        if rows[i].kind != Kind::Equal {
            items.push(Item::Row(i));
            i += 1;
            continue;
        }
        let start = i;
        while i < rows.len() && rows[i].kind == Kind::Equal { i += 1; }
        let end = i;
        // Context after the previous change and before the next one (none at the ends).
        let hidden_start = if start == 0 { 0 } else { start + CONTEXT };
        let hidden_end = if end == rows.len() { end } else { end.saturating_sub(CONTEXT) };
        if collapse && hidden_end >= hidden_start + MIN_FOLD && !expanded.contains(&hidden_start) {
            items.extend((start..hidden_start).map(Item::Row));
            items.push(Item::Fold { start: hidden_start, end: hidden_end });
            items.extend((hidden_end..end).map(Item::Row));
        } else {
            items.extend((start..end).map(Item::Row));
        }
    }
    items
}

/// Positions (in `items`) where a block of changes starts.
fn change_starts(items: &[Item], rows: &[Row]) -> Vec<usize> {
    let is_change = |item: &Item| matches!(item, Item::Row(r) if rows[*r].kind != Kind::Equal);
    (0..items.len()).filter(|&i| is_change(&items[i]) && (i == 0 || !is_change(&items[i - 1]))).collect()
}

/// Gives the parts of `job` inside `ranges` a background, splitting sections where needed.
fn mark_words(job: &mut egui::text::LayoutJob, ranges: &[Range<usize>], color: egui::Color32) {
    let mut sections = Vec::with_capacity(job.sections.len() + 2 * ranges.len());
    for section in job.sections.drain(..) {
        let (start, end) = (section.byte_range.start, section.byte_range.end);
        let mut cuts = vec![start, end];
        cuts.extend(ranges.iter().flat_map(|r| [r.start, r.end]).filter(|&b| b > start && b < end));
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let mut part = section.clone();
            part.byte_range = pair[0]..pair[1];
            if pair[0] != start { part.leading_space = 0.0; }
            if ranges.iter().any(|r| r.start <= pair[0] && pair[1] <= r.end) { part.format.background = color; }
            sections.push(part);
        }
    }
    job.sections = sections;
}

/// How a line is drawn: its background, sign and changed-word color.
#[derive(Clone, Copy, PartialEq)]
enum Tone { Plain, Removed, Added, Filler }

/// What every line of a column needs to draw itself.
struct LineStyle<'a> { language: &'a str, font: egui::FontId, char_width: f32, digits: usize, gutter: f32 }

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
    let left = ui.clip_rect().left().max(rect.left());
    let gutter = egui::Rect::from_min_size(egui::pos2(left, rect.top()), egui::vec2(style.gutter, rect.height()));
    painter.rect_filled(gutter, 0.0, theme::GUTTER);
    painter.rect_filled(gutter, 0.0, row_bg);
    let number_width = style.digits as f32 * style.char_width + 10.0;
    for (i, number) in numbers.iter().enumerate() {
        let Some(number) = number else { continue };
        let at = egui::pos2(gutter.left() + (i + 1) as f32 * number_width - 2.0, rect.center().y);
        painter.text(at, egui::Align2::RIGHT_CENTER, number.to_string(), style.font.clone(), theme::SUBTLE);
    }
    painter.text(egui::pos2(gutter.right() - 9.0, rect.center().y), egui::Align2::CENTER_CENTER, sign, style.font.clone(), sign_color);
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

fn row_height(ui: &egui::Ui) -> f32 { ui.text_style_height(&egui::TextStyle::Monospace) + 5.0 }

pub struct CodeCompare {
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
    collapse: bool,
    show_texts: bool,
    result: Comparison,
    items: Vec<Item>,
    /// Folds the user opened, by the index of their first row.
    expanded: HashSet<usize>,
    /// Vertical scroll position, shared by both columns.
    scroll_y: f32,
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
    fn recompare(&mut self) {
        self.result = compare(&self.texts[0], &self.texts[1], &self.names, self.options);
        self.detected = self.names.iter().find_map(|name| language_of_file(name)).unwrap_or_else(|| {
            let language = sniff_language(&self.texts[1]);
            if language.is_empty() { sniff_language(&self.texts[0]) } else { language }
        });
        self.expanded.clear();
        self.relayout();
    }

    fn rows(&self) -> &[Row] {
        match self.view { View::SideBySide => &self.result.rows, View::Unified => &self.result.unified }
    }

    /// Rebuilds the visible lines after the view, folding or the comparison changed.
    fn relayout(&mut self) {
        self.items = fold(self.rows(), self.collapse, &self.expanded);
    }

    fn language(&self) -> &'static str { self.language.map_or(self.detected, |i| LANGUAGES[i].1) }

    /// Loads files into the sides from `side` on (two files fill both sides).
    fn load_files(&mut self, side: usize, files: Vec<Result<(String, Vec<u8>), String>>) {
        for (side, file) in (side..2).zip(files) {
            match file.and_then(|(name, bytes)| String::from_utf8(bytes).map(|text| (name.clone(), text)).map_err(|_| format!("{name} is not a UTF-8 text file"))) {
                Ok((name, text)) => { self.texts[side] = text; self.names[side] = name; self.error.clear(); }
                Err(e) => self.error = e,
            }
        }
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
            relayout |= ui::segmented(ui, &mut self.view, &[(View::SideBySide, side.as_str()), (View::Unified, unified.as_str())]);
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
        let numbers = if side.is_some() { 1.0 } else { 2.0 };
        let gutter = numbers * (digits as f32 * char_width + 10.0) + 18.0;
        let style = LineStyle { language: self.language(), font, char_width, digits, gutter };
        let widest = side.map_or(r.widest[0].max(r.widest[1]), |s| r.widest[s]);
        let content_width = gutter + widest as f32 * char_width + 32.0;
        let row_height = row_height(ui);
        ui::editor_frame().inner_margin(1).show(ui, |ui| {
            let width = ui.available_width();
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
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

    fn run(old: &str, new: &str, options: Options) -> Comparison { compare(old, new, &Default::default(), options) }

    #[test]
    fn marks_changed_words() {
        let r = run("let a = 1;\nlet b = 2;\n", "let a = 1;\nlet b = 3;\nlet c = 4;\n", Options::default());
        let kinds: Vec<Kind> = r.rows.iter().map(|row| row.kind).collect();
        assert_eq!(kinds, [Kind::Equal, Kind::Changed, Kind::Added]);
        let changed = &r.rows[1];
        let (left, right) = (changed.left.as_ref().unwrap(), changed.right.as_ref().unwrap());
        assert_eq!(left.words.iter().map(|w| &r.old[1][w.clone()]).collect::<Vec<_>>(), ["2;"]);
        assert_eq!(right.words.iter().map(|w| &r.new[1][w.clone()]).collect::<Vec<_>>(), ["3;"]);
        assert_eq!((r.added, r.removed), (2, 1));
        assert!(r.patch.contains("-let b = 2;\n+let b = 3;\n+let c = 4;\n"), "{}", r.patch);
    }

    #[test]
    fn ignores_whitespace_and_case_on_request() {
        assert_eq!(run("a  b\nc", "a b\nC", Options::default()).removed, 2);
        let r = run("a  b\nc", "a b\nC", Options { ignore_whitespace: true, ignore_case: true });
        assert_eq!((r.added, r.removed), (0, 0));
        assert!(r.patch.is_empty());
    }

    #[test]
    fn pairs_similar_lines_in_a_block_of_changes() {
        // The old `return` line pairs with the new one, not with the inserted call before it.
        let r = run("x\n  return 'Hello, ' + user.name;\ny\n", "x\n  logVisit(user.id);\n  return `Hello, ${user.name}`;\ny\n", Options::default());
        let kinds: Vec<Kind> = r.rows.iter().map(|row| row.kind).collect();
        assert_eq!(kinds, [Kind::Equal, Kind::Added, Kind::Changed, Kind::Equal]);
        assert_eq!(r.rows[2].left.as_ref().map(|c| c.line), Some(1));
        assert_eq!(r.rows[2].right.as_ref().map(|c| c.line), Some(2));
        // Without similar lines, lines are paired in order.
        assert_eq!(pair_lines(&["alpha beta".into()], &["12345".into(), "xyz".into()]), [(Some(0), Some(0)), (None, Some(1))]);
        assert!(similarity("let x = 1;", "let x = 2;") > 0.7);
    }

    #[test]
    fn unified_view_lists_removed_lines_first() {
        let r = run("x\none\ntwo\ny", "x\nuno\ndos\ny", Options::default());
        let kinds: Vec<Kind> = r.unified.iter().map(|row| row.kind).collect();
        assert_eq!(kinds, [Kind::Equal, Kind::Removed, Kind::Removed, Kind::Added, Kind::Added, Kind::Equal]);
    }

    #[test]
    fn folds_long_unchanged_runs() {
        let old: String = (0..30).map(|i| format!("line {i}\n")).collect();
        let new = old.replace("line 15\n", "changed\n");
        let r = run(&old, &new, Options::default());
        let items = fold(&r.rows, true, &HashSet::new());
        assert_eq!(items.first(), Some(&Item::Fold { start: 0, end: 12 }));
        assert_eq!(items.last(), Some(&Item::Fold { start: 19, end: 30 }));
        assert_eq!(items.len(), 1 + 3 + 1 + 3 + 1);
        assert_eq!(change_starts(&items, &r.rows), [4]);
        // An opened fold shows its lines again; folding off shows everything.
        assert_eq!(fold(&r.rows, true, &HashSet::from([0])).len(), 12 + 3 + 1 + 3 + 1);
        assert_eq!(fold(&r.rows, false, &HashSet::new()).len(), 30);
    }

    #[test]
    fn handles_large_inputs() {
        let old: String = (0..20_000).map(|i| format!("line {i}\n")).collect();
        let new = old.replace("line 10000\n", "changed\n");
        let r = run(&old, &new, Options::default());
        assert_eq!((r.added, r.removed), (1, 1));
    }

    #[test]
    fn detects_languages() {
        assert_eq!(language_of_file("src/App.tsx"), Some("js"));
        assert_eq!(language_of_file("Cargo.lock"), None);
        assert_eq!(sniff_language("{\"a\": 1}"), "json");
        assert_eq!(sniff_language("select * from users"), "sql");
        assert_eq!(sniff_language("fn main() {\n    let x = 1;\n}"), "rs");
        assert_eq!(sniff_language(SAMPLE_MODIFIED), "js");
        assert_eq!(sniff_language("just some words"), "");
    }

    #[test]
    fn marks_words_inside_highlighted_sections() {
        let mut job = egui::text::LayoutJob::simple_singleline("let b = 2;".into(), egui::FontId::monospace(12.0), theme::TEXT);
        mark_words(&mut job, std::slice::from_ref(&(8..10)), theme::MATCH_BG);
        let parts: Vec<(Range<usize>, bool)> = job.sections.iter().map(|s| (s.byte_range.clone(), s.format.background == theme::MATCH_BG)).collect();
        assert_eq!(parts, [(0..8, false), (8..10, true)]);
    }
}
