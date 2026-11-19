//! The comparison behind Code Compare: a line diff (by key, so whitespace or
//! case can be ignored), similar lines paired side by side, the changed words
//! of each modified line, the unified (patch) order, and folding.
use std::collections::HashSet;
use std::ops::Range;
use std::time::Duration;
use similar::{ChangeTag, DiffOp};

/// Unchanged lines kept visible around each change when folding.
const CONTEXT: usize = 3;
/// Shorter unchanged stretches are never folded.
const MIN_FOLD: usize = 4;
/// Time allowed for the line diff and for the patch, after which they settle
/// for a larger diff than the smallest one.
const LINE_DIFF_TIMEOUT: Duration = Duration::from_millis(500);
/// Time allowed for the word diff of one modified line.
const WORD_DIFF_TIMEOUT: Duration = Duration::from_millis(20);

/// What the line diff ignores.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Options {
    /// Compares lines with all their whitespace removed (like `diff -w`).
    pub(super) ignore_whitespace: bool,
    pub(super) ignore_case: bool,
}

/// What happened to the line(s) of a [`Row`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Equal,
    /// Only in the original text.
    Removed,
    /// Only in the modified text.
    Added,
    /// Modified: a line of each text, with their changed words.
    Changed,
}

/// One side of a row: the line's index in its text and the byte ranges of its changed words.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Cell { pub(super) line: usize, pub(super) words: Vec<Range<usize>> }

impl Cell {
    /// The cell of line `line`, with no changed words marked.
    fn new(line: usize) -> Self { Cell { line, words: Vec::new() } }
}

/// A row of the view. `Changed` rows have both cells, `Removed` only the
/// left one and `Added` only the right one.
#[derive(Clone, Debug)]
pub(super) struct Row {
    pub(super) kind: Kind,
    /// The original line (`None` for an added line).
    pub(super) left: Option<Cell>,
    /// The modified line (`None` for a removed line).
    pub(super) right: Option<Cell>,
}

impl Row {
    /// An unchanged line, at index `old` in the original and `new` in the modified text.
    fn equal(old: usize, new: usize) -> Self { Row { kind: Kind::Equal, left: Some(Cell::new(old)), right: Some(Cell::new(new)) } }
    /// A line only in the original text.
    fn removed(old: usize) -> Self { Row { kind: Kind::Removed, left: Some(Cell::new(old)), right: None } }
    /// A line only in the modified text.
    fn added(new: usize) -> Self { Row { kind: Kind::Added, left: None, right: Some(Cell::new(new)) } }
}

/// A line of the view: a row, or a run of unchanged rows folded into one line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Item {
    /// Index of the row in the view's rows.
    Row(usize),
    /// Rows `start..end`, hidden behind one line that opens them when clicked.
    Fold { start: usize, end: usize },
}

/// The result of [`compare`]: the lines of both texts and how they line up.
#[derive(Default)]
pub(super) struct Comparison {
    /// Lines of the original text, tabs expanded.
    pub(super) old: Vec<String>,
    /// Lines of the modified text, tabs expanded.
    pub(super) new: Vec<String>,
    /// Side-by-side rows: a modified line has both cells.
    pub(super) rows: Vec<Row>,
    /// The same changes as in a patch: removed lines, then the added ones.
    pub(super) unified: Vec<Row>,
    /// Lines a patch marks `+`: added and modified lines.
    pub(super) added: usize,
    /// Lines a patch marks `-`: removed and modified lines.
    pub(super) removed: usize,
    /// Longest line of each side, in characters (sets the horizontal scroll range).
    pub(super) widest: [usize; 2],
    /// Standard unified diff, for the Copy button.
    pub(super) patch: String,
}

/// Lines of `text`, with tabs expanded so that they line up when drawn.
fn split_lines(text: &str) -> Vec<String> {
    text.lines().map(|line| line.replace('\t', "    ")).collect()
}

/// Compares two texts line by line, then word by word within modified lines.
/// Gives up on the smallest diff after a short deadline, so huge inputs stay responsive.
/// `names` are the file names for the patch header (empty when typed).
pub(super) fn compare(old_text: &str, new_text: &str, names: &[String; 2], options: Options) -> Comparison {
    let (old, new) = (split_lines(old_text), split_lines(new_text));
    let key = |line: &String| {
        let line = if options.ignore_whitespace { line.split_whitespace().collect() } else { line.clone() };
        if options.ignore_case { line.to_lowercase() } else { line }
    };
    let deadline = std::time::Instant::now() + LINE_DIFF_TIMEOUT;
    let ops = similar::capture_diff_slices_by_key_deadline(similar::Algorithm::Myers, &old, &new, key, Some(deadline));
    let mut rows = Vec::new();
    for op in ops {
        match op {
            DiffOp::Equal { old_index, new_index, len } => rows.extend((0..len).map(|i| Row::equal(old_index + i, new_index + i))),
            DiffOp::Delete { old_index, old_len, .. } => rows.extend((old_index..old_index + old_len).map(Row::removed)),
            DiffOp::Insert { new_index, new_len, .. } => rows.extend((new_index..new_index + new_len).map(Row::added)),
            // A block of lines replaced by another: pair the similar ones as modified lines.
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                for pair in pair_lines(&old[old_index..old_index + old_len], &new[new_index..new_index + new_len]) {
                    rows.push(match pair {
                        (Some(o), Some(n)) => {
                            let (o, n) = (old_index + o, new_index + n);
                            let (left, right) = word_changes(&old[o], &new[n]);
                            Row { kind: Kind::Changed, left: Some(Cell { line: o, words: left }), right: Some(Cell { line: n, words: right }) }
                        }
                        (Some(o), None) => Row::removed(old_index + o),
                        // `pair_lines` never returns two `None`s.
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
    // The patch is of the texts as they are: the ignore options and tab expansion do not apply.
    let patch = if added + removed == 0 { String::new() } else {
        let header = |i: usize, fallback: &'static str| if names[i].is_empty() { fallback.to_owned() } else { names[i].clone() };
        similar::TextDiff::configure().timeout(LINE_DIFF_TIMEOUT).diff_lines(old_text, new_text)
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
    // Walk the table along an optimal path (the float comparisons are exact: they
    // repeat the sums the table was built from). Lines left between two similar
    // pairs are zipped side by side.
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
    let diff = similar::TextDiff::configure().timeout(WORD_DIFF_TIMEOUT).diff_words(old, new);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    // Byte offsets in the old and new line, advanced by each change of their side.
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
    // Added lines wait here until the block of changes ends.
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
pub(super) fn fold(rows: &[Row], collapse: bool, expanded: &HashSet<usize>) -> Vec<Item> {
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
pub(super) fn change_starts(items: &[Item], rows: &[Row]) -> Vec<usize> {
    let is_change = |item: &Item| matches!(item, Item::Row(r) if rows[*r].kind != Kind::Equal);
    (0..items.len()).filter(|&i| is_change(&items[i]) && (i == 0 || !is_change(&items[i - 1]))).collect()
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
        assert_eq!(r.patch, "");
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
}
