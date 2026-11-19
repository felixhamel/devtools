//! Password Generator: random passwords or word passphrases from the OS secure
//! random generator, with a strength estimate.
use eframe::egui;
use super::{text::random_bytes, Category, Tool};
use crate::ui::{self, icon, theme};

/// Character class: lowercase letters.
const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
/// Character class: uppercase letters.
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// Character class: digits.
const DIGITS: &str = "0123456789";
/// Character class: common ASCII symbols (no quotes, backslash or space).
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?/~";
/// Characters easily confused when read or typed: 0/O/o, 1/l/I, and quote-like ones.
const AMBIGUOUS: &str = "0Oo1lI|`'\"";

/// What the generator makes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    /// Random characters from the selected classes.
    Password,
    /// Random words from the BIP-39 list.
    Passphrase,
}

/// The generator's settings; results are regenerated whenever they change.
#[derive(Clone, PartialEq, Debug)]
struct Options {
    mode: Mode,
    /// Characters per password.
    length: usize,
    /// Include [`LOWER`].
    lower: bool,
    /// Include [`UPPER`].
    upper: bool,
    /// Include [`DIGITS`].
    digits: bool,
    /// Include [`SYMBOLS`].
    symbols: bool,
    /// Leave out the [`AMBIGUOUS`] characters.
    avoid_ambiguous: bool,
    /// Words per passphrase.
    words: usize,
    /// Put between passphrase words (may be empty).
    separator: String,
    /// Start each passphrase word with a capital.
    capitalize: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { mode: Mode::Password, length: 20, lower: true, upper: true, digits: true, symbols: true, avoid_ambiguous: false, words: 5, separator: "-".into(), capitalize: true }
    }
}

impl Options {
    /// The selected character classes, without ambiguous characters if asked.
    fn classes(&self) -> Vec<Vec<char>> {
        [(self.lower, LOWER), (self.upper, UPPER), (self.digits, DIGITS), (self.symbols, SYMBOLS)].into_iter()
            .filter(|(on, _)| *on)
            .map(|(_, set)| set.chars().filter(|c| !(self.avoid_ambiguous && AMBIGUOUS.contains(*c))).collect())
            .collect()
    }

    /// Bits of entropy of one result: what an attacker who knows these options must guess.
    fn entropy_bits(&self) -> f64 {
        match self.mode {
            Mode::Password => {
                let alphabet: usize = self.classes().iter().map(Vec::len).sum();
                if alphabet == 0 { 0.0 } else { self.length as f64 * (alphabet as f64).log2() }
            }
            Mode::Passphrase => self.words as f64 * 11.0, // 2048 words = 11 bits each
        }
    }
}

/// A uniform random index in `0..n`, rejecting values that would bias the result.
fn random_index(n: usize) -> Result<usize, String> {
    let n = u32::try_from(n).map_err(|_| "Range too large")?;
    let zone = u32::MAX - (u32::MAX % n); // largest multiple of n
    loop {
        let value = u32::from_le_bytes(random_bytes::<4>()?);
        if value < zone { return Ok((value % n) as usize); }
    }
}

/// A password with at least one character from each selected class, shuffled.
fn password(options: &Options) -> Result<String, String> {
    let classes = options.classes();
    if classes.is_empty() { return Err("Select at least one kind of character".into()); }
    if options.length < classes.len() { return Err(format!("Use at least {} characters to include every selected kind", classes.len())); }
    let all: Vec<char> = classes.concat();
    let mut chars: Vec<char> = classes.iter().map(|class| random_index(class.len()).map(|i| class[i])).collect::<Result<_, _>>()?;
    while chars.len() < options.length { chars.push(all[random_index(all.len())?]); }
    // Fisher–Yates, so the guaranteed characters are not always first.
    for i in (1..chars.len()).rev() { chars.swap(i, random_index(i + 1)?); }
    Ok(chars.into_iter().collect())
}

/// Words from the BIP-39 English list (2048 common words).
fn passphrase(options: &Options) -> Result<String, String> {
    let list = bip39::Language::English.word_list();
    let words: Vec<String> = (0..options.words).map(|_| {
        let word = list[random_index(list.len())?];
        Ok(if options.capitalize { super::text::capitalize(word) } else { word.to_owned() })
    }).collect::<Result<_, String>>()?;
    Ok(words.join(&options.separator))
}

/// Label and color for `bits` of entropy.
fn strength(bits: f64) -> (&'static str, egui::Color32) {
    match bits {
        b if b < 45.0 => ("Weak", theme::DIFF_REMOVED),
        b if b < 70.0 => ("Fair", theme::WARNING),
        b if b < 100.0 => ("Strong", theme::DIFF_ADDED),
        _ => ("Very strong", theme::ACCENT),
    }
}

/// The Password Generator tool.
pub struct Password {
    options: Options,
    /// How many passwords to generate.
    count: usize,
    /// The results, one per line.
    output: String,
    error: String,
    /// Options and count `output` was generated with; a change regenerates.
    generated_for: Option<(Options, usize)>,
}

impl Default for Password {
    fn default() -> Self { Self { options: Options::default(), count: 5, output: String::new(), error: String::new(), generated_for: None } }
}

impl Password {
    /// Replaces the output with `count` new results (or the error).
    fn generate(&mut self) {
        let make = |o: &Options| match o.mode { Mode::Password => password(o), Mode::Passphrase => passphrase(o) };
        match (0..self.count).map(|_| make(&self.options)).collect::<Result<Vec<_>, _>>() {
            Ok(list) => { self.output = list.join("\n"); self.error.clear(); }
            Err(e) => { self.output.clear(); self.error = e; }
        }
        self.generated_for = Some((self.options.clone(), self.count));
    }

    /// The mode switch, the options of that mode, the count and the strength meter.
    fn options_ui(&mut self, ui: &mut egui::Ui) {
        let o = &mut self.options;
        let (password, passphrase) = (format!("{}  Password", icon::PASSWORD), format!("{}  Passphrase", icon::TEXT_AA));
        ui::segmented(ui, &mut o.mode, &[(Mode::Password, password.as_str()), (Mode::Passphrase, passphrase.as_str())]);
        ui.add_space(10.0);
        egui::Grid::new("password-options").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
            match o.mode {
                Mode::Password => {
                    ui.label(egui::RichText::new("Length").color(theme::MUTED));
                    ui.add(egui::Slider::new(&mut o.length, 4..=128));
                    ui.end_row();
                    ui.label(egui::RichText::new("Characters").color(theme::MUTED));
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut o.lower, "a–z");
                        ui.checkbox(&mut o.upper, "A–Z");
                        ui.checkbox(&mut o.digits, "0–9");
                        ui.checkbox(&mut o.symbols, "!@#$…");
                    });
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut o.avoid_ambiguous, "Avoid look-alikes (0 O 1 l I…)");
                    ui.end_row();
                }
                Mode::Passphrase => {
                    ui.label(egui::RichText::new("Words").color(theme::MUTED));
                    ui.add(egui::Slider::new(&mut o.words, 3..=12));
                    ui.end_row();
                    ui.label(egui::RichText::new("Separator").color(theme::MUTED));
                    ui.add(egui::TextEdit::singleline(&mut o.separator).desired_width(40.0).font(egui::TextStyle::Monospace));
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut o.capitalize, "Capitalize words");
                    ui.end_row();
                }
            }
            ui.label(egui::RichText::new("How many").color(theme::MUTED));
            ui.add(egui::Slider::new(&mut self.count, 1..=50));
            ui.end_row();
        });
        ui.add_space(14.0);
        let bits = self.options.entropy_bits();
        let (label, color) = strength(bits);
        ui.label(egui::RichText::new(format!("Strength: {label} · about {bits:.0} bits of entropy")).color(color));
        // 128 bits (beyond brute force) fills the bar.
        ui.add(egui::ProgressBar::new((bits / 128.0).min(1.0) as f32).fill(color).desired_height(6.0));
    }
}

impl Tool for Password {
    fn name(&self) -> &'static str { "Password Generator" }
    fn icon(&self) -> &'static str { icon::PASSWORD }
    fn category(&self) -> Category { Category::Security }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Generated with the operating system's secure random generator. Nothing is stored.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut regenerate = false;
            ui::toolbar(left, "Options:", |ui| {
                regenerate |= ui.add(ui::icon_button(icon::ARROWS_CLOCKWISE, "Generate", true)).clicked();
            });
            self.options_ui(left);
            // New results whenever an option changes (and on first display).
            regenerate |= self.generated_for.as_ref() != Some(&(self.options.clone(), self.count));
            if regenerate { self.generate(); }
            ui::toolbar(right, "Generated:", |ui| ui::copy_button(ui, &self.output));
            ui::error_label(right, &self.error);
            ui::fill_editor(right, "passwords", &mut self.output, "");
        });
    }

    fn load_sample(&mut self) { self.generate(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_respect_options() {
        let options = Options { length: 12, symbols: false, avoid_ambiguous: true, ..Options::default() };
        for _ in 0..200 {
            let p = password(&options).unwrap();
            assert_eq!(p.chars().count(), 12);
            assert!(p.chars().all(|c| c.is_ascii_alphanumeric() && !AMBIGUOUS.contains(c)), "{p}");
            // Every selected class appears at least once.
            assert!(p.chars().any(|c| c.is_ascii_lowercase()) && p.chars().any(|c| c.is_ascii_uppercase()) && p.chars().any(|c| c.is_ascii_digit()), "{p}");
        }
        assert!(password(&Options { lower: false, upper: false, digits: false, symbols: false, ..Options::default() }).is_err());
        assert!(password(&Options { length: 3, ..Options::default() }).is_err());
    }

    #[test]
    fn passphrases_use_the_word_list() {
        let options = Options { mode: Mode::Passphrase, words: 6, separator: ".".into(), capitalize: false, ..Options::default() };
        let phrase = passphrase(&options).unwrap();
        let list = bip39::Language::English.word_list();
        assert_eq!(phrase.split('.').count(), 6);
        assert!(phrase.split('.').all(|w| list.contains(&w)), "{phrase}");
    }

    #[test]
    fn entropy_and_uniformity() {
        assert_eq!(Options { mode: Mode::Passphrase, words: 5, ..Options::default() }.entropy_bits(), 55.0);
        let digits_only = Options { length: 10, lower: false, upper: false, symbols: false, ..Options::default() };
        assert!((digits_only.entropy_bits() - 10.0 * 10f64.log2()).abs() < 1e-9);
        // Every index of a small range comes up.
        let mut seen = [false; 3];
        for _ in 0..300 { seen[random_index(3).unwrap()] = true; }
        assert!(seen.iter().all(|s| *s));
    }
}
