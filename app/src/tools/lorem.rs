//! Lorem Ipsum Generator: placeholder paragraphs, sentences or words in
//! pseudo-Latin, optionally starting with the classic "Lorem ipsum…".
use eframe::egui;
use super::{text::random_bytes, Category, Tool};
use crate::ui::{self, icon, theme};

/// The classic opening sentence, without its final period.
const OPENING: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit";

/// Words the text is made of, from the traditional passage of Cicero's
/// *De finibus* and its usual variations.
const WORDS: [&str; 120] = [
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed", "do", "eiusmod", "tempor",
    "incididunt", "ut", "labore", "et", "dolore", "magna", "aliqua", "enim", "ad", "minim", "veniam", "quis",
    "nostrud", "exercitation", "ullamco", "laboris", "nisi", "aliquip", "ex", "ea", "commodo", "consequat", "duis", "aute",
    "irure", "in", "reprehenderit", "voluptate", "velit", "esse", "cillum", "fugiat", "nulla", "pariatur", "excepteur", "sint",
    "occaecat", "cupidatat", "non", "proident", "sunt", "culpa", "qui", "officia", "deserunt", "mollit", "anim", "id",
    "est", "laborum", "curabitur", "pretium", "tincidunt", "lacus", "gravida", "orci", "a", "odio", "nullam", "varius",
    "turpis", "pharetra", "eros", "bibendum", "nec", "luctus", "felis", "sollicitudin", "mauris", "integer", "eu", "nibh",
    "euismod", "ac", "tellus", "risus", "vulputate", "vehicula", "donec", "lobortis", "etiam", "ullamcorper", "ligula", "congue",
    "sapien", "quam", "maecenas", "fermentum", "mi", "pellentesque", "malesuada", "sem", "aliquet", "eget", "neque", "aliquam",
    "faucibus", "dictum", "nisl", "diam", "erat", "cras", "scelerisque", "nunc", "arcu", "augue", "dapibus", "laoreet",
];

/// What the count counts.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Unit { Paragraphs, Sentences, Words }

impl Unit {
    /// Every unit, in the order of the switch.
    const ALL: [Unit; 3] = [Unit::Paragraphs, Unit::Sentences, Unit::Words];

    /// Label of the switch.
    fn label(self) -> &'static str {
        match self { Unit::Paragraphs => "Paragraphs", Unit::Sentences => "Sentences", Unit::Words => "Words" }
    }

    /// The most the count slider allows.
    fn max(self) -> usize {
        match self { Unit::Paragraphs => 50, Unit::Sentences => 200, Unit::Words => 2_000 }
    }
}

/// The generator's settings; the text is regenerated whenever they change.
#[derive(Clone, PartialEq, Debug)]
struct Options {
    unit: Unit,
    /// How many of each unit, kept separately so switching units keeps a sensible count.
    counts: [usize; 3],
    /// Begin with "Lorem ipsum dolor sit amet…".
    classic_start: bool,
    /// Wrap paragraphs in `<p>` tags.
    html: bool,
}

impl Default for Options {
    fn default() -> Self { Self { unit: Unit::Paragraphs, counts: [3, 5, 50], classic_start: true, html: false } }
}

impl Options {
    /// How many units to generate.
    fn count(&self) -> usize { self.counts[self.unit as usize] }
}

/// A small xorshift generator: placeholder text needs variety, not security,
/// and a fixed seed makes the tests repeatable.
struct Rng(u64);

impl Rng {
    /// Seeded from the operating system's random generator.
    fn new() -> Self { Self::seeded(random_bytes::<8>().map(u64::from_le_bytes).unwrap_or(0x2545_F491_4F6C_DD1D)) }

    /// Seeded with `seed` (xorshift needs a non-zero state).
    fn seeded(seed: u64) -> Self { Self(seed.max(1)) }

    /// A number in `low..=high`.
    fn between(&mut self, low: usize, high: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        low + (self.0 % (high - low + 1) as u64) as usize
    }

    /// A random word of [`WORDS`].
    fn word(&mut self) -> &'static str { WORDS[self.between(0, WORDS.len() - 1)] }
}

/// A capitalized sentence of 6 to 14 words, sometimes with a comma, ending with a period.
fn sentence(rng: &mut Rng) -> String {
    let length = rng.between(6, 14);
    let mut words: Vec<String> = (0..length).map(|_| rng.word().to_owned()).collect();
    // About one comma per long sentence, never after the first two or the last two words.
    if length >= 9 { words[rng.between(2, length - 3)].push(','); }
    format!("{}.", super::text::capitalize(&words.join(" ")))
}

/// `count` sentences, the first one classic if asked.
fn sentences(rng: &mut Rng, count: usize, classic_start: bool) -> Vec<String> {
    (0..count).map(|i| if i == 0 && classic_start { format!("{OPENING}.") } else { sentence(rng) }).collect()
}

/// The text for `options`, from `rng`.
fn generate(options: &Options, rng: &mut Rng) -> String {
    let count = options.count();
    match options.unit {
        Unit::Words => {
            let classic = OPENING.to_lowercase().replace(',', "");
            let start: Vec<&str> = if options.classic_start { classic.split(' ').collect() } else { Vec::new() };
            let words: Vec<&str> = start.into_iter().chain(std::iter::repeat_with(|| rng.word())).take(count).collect();
            let text = words.join(" ");
            if options.classic_start { super::text::capitalize(&text) } else { text }
        }
        Unit::Sentences => sentences(rng, count, options.classic_start).join(" "),
        Unit::Paragraphs => (0..count).map(|i| {
            // 4 to 8 sentences per paragraph.
            let length = rng.between(4, 8);
            let paragraph = sentences(rng, length, i == 0 && options.classic_start).join(" ");
            if options.html { format!("<p>{paragraph}</p>") } else { paragraph }
        }).collect::<Vec<_>>().join("\n\n"),
    }
}

/// The Lorem Ipsum Generator tool.
#[derive(Default)]
pub struct Lorem {
    options: Options,
    output: String,
    /// The options `output` was generated with; a change regenerates.
    generated_for: Option<Options>,
}

impl Lorem {
    /// Replaces the output with new text.
    fn generate(&mut self) {
        self.output = generate(&self.options, &mut Rng::new());
        self.generated_for = Some(self.options.clone());
    }

    /// The unit switch, the count and the options.
    fn options_ui(&mut self, ui: &mut egui::Ui) {
        let o = &mut self.options;
        let units: Vec<(Unit, &str)> = Unit::ALL.iter().map(|u| (*u, u.label())).collect();
        ui::segmented(ui, &mut o.unit, &units);
        ui.add_space(10.0);
        egui::Grid::new("lorem-options").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
            ui.label(egui::RichText::new("How many").color(theme::MUTED));
            let max = o.unit.max();
            ui.add(egui::Slider::new(&mut o.counts[o.unit as usize], 1..=max).logarithmic(max > 100));
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut o.classic_start, "Start with \u{201c}Lorem ipsum dolor sit amet\u{2026}\u{201d}");
            ui.end_row();
            if o.unit == Unit::Paragraphs {
                ui.label("");
                ui.checkbox(&mut o.html, "Wrap paragraphs in <p> tags");
                ui.end_row();
            }
        });
    }
}

impl Tool for Lorem {
    fn name(&self) -> &'static str { "Lorem Ipsum Generator" }
    fn icon(&self) -> &'static str { icon::PARAGRAPH }
    fn category(&self) -> Category { Category::Text }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Placeholder text for layouts and tests: paragraphs, sentences or words.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut regenerate = false;
            ui::toolbar(left, "Options:", |ui| {
                regenerate |= ui.add(ui::icon_button(icon::ARROWS_CLOCKWISE, "Generate", true)).clicked();
            });
            self.options_ui(left);
            // New text whenever an option changes (and on first display).
            regenerate |= self.generated_for.as_ref() != Some(&self.options);
            if regenerate { self.generate(); }
            ui::toolbar(right, "Generated:", |ui| {
                ui::copy_button(ui, &self.output);
                let words = self.output.split_whitespace().count();
                ui.label(egui::RichText::new(format!("{words} words")).small().color(theme::SUBTLE));
            });
            ui::fill_editor(right, "lorem", &mut self.output, "");
        });
    }

    fn load_sample(&mut self) { self.generate(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_each_unit() {
        let mut rng = Rng::seeded(42);
        let words = Options { unit: Unit::Words, counts: [0, 0, 12], ..Options::default() };
        let text = generate(&words, &mut rng);
        assert!(text.starts_with("Lorem ipsum dolor sit amet consectetur adipiscing elit "), "{text}");
        assert_eq!(text.split(' ').count(), 12);
        let sentences = Options { unit: Unit::Sentences, counts: [0, 4, 0], classic_start: false, ..Options::default() };
        let text = generate(&sentences, &mut rng);
        assert_eq!(text.matches(". ").count() + 1, 4, "{text}");
        assert!(text.ends_with('.') && text.starts_with(char::is_uppercase), "{text}");
        let paragraphs = Options { counts: [3, 0, 0], html: true, ..Options::default() };
        let text = generate(&paragraphs, &mut rng);
        assert!(text.starts_with("<p>Lorem ipsum dolor sit amet, consectetur adipiscing elit. "), "{text}");
        assert_eq!(text.split("\n\n").count(), 3);
        assert!(text.split("\n\n").all(|p| p.starts_with("<p>") && p.ends_with(".</p>")), "{text}");
    }

    #[test]
    fn options_and_seeds() {
        let mut rng = Rng::seeded(3);
        // One classic sentence is exactly the opening; <p> only wraps paragraphs.
        let one = Options { unit: Unit::Sentences, counts: [0, 1, 0], html: true, ..Options::default() };
        assert_eq!(generate(&one, &mut rng), format!("{OPENING}."));
        let words = Options { unit: Unit::Words, counts: [0, 0, 3], classic_start: false, ..Options::default() };
        let text = generate(&words, &mut rng);
        assert!(text.split(' ').count() == 3 && text.split(' ').all(|w| WORDS.contains(&w)), "{text}");
        // The same seed gives the same text.
        assert_eq!(generate(&Options::default(), &mut Rng::seeded(9)), generate(&Options::default(), &mut Rng::seeded(9)));
    }

    #[test]
    fn sentences_are_well_formed() {
        let mut rng = Rng::seeded(7);
        for _ in 0..200 {
            let s = sentence(&mut rng);
            let words = s.split(' ').count();
            assert!((6..=14).contains(&words), "{s}");
            assert!(s.ends_with('.') && !s.ends_with(",.") && s.starts_with(char::is_uppercase), "{s}");
        }
        // Fewer words than the classic opening keeps only its start.
        let short = Options { unit: Unit::Words, counts: [0, 0, 2], ..Options::default() };
        assert_eq!(generate(&short, &mut rng), "Lorem ipsum");
    }
}
