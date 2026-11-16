//! The tool registry.
//!
//! Adding a tool:
//! - text in, text out: build a [`TextTool`] with one or more transform functions;
//! - anything else: implement [`Tool`] on a struct that holds the tool's state.
//!
//! Then add it to [`registry()`]. Each tool keeps its own state while the app runs,
//! so switching tools does not lose what was typed.
use eframe::egui;
use crate::ui::{self, TextIo};

mod base64_image;
mod code;
mod cron;
mod curl;
mod data;
mod date;
mod diff;
mod encoding;
mod format;
mod identifiers;
mod json_code;
mod lines;
mod markdown;
mod qr;
mod regex_tester;
mod security;
mod text;
mod x509;

/// Sidebar groups, shown in the order of [`Category::ALL`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category { Date, Data, Encoding, Security, Text, Code }

impl Category {
    pub const ALL: [Category; 6] = [Category::Date, Category::Data, Category::Encoding, Category::Security, Category::Text, Category::Code];

    pub fn label(self) -> &'static str {
        match self {
            Category::Date => "Date & scheduling",
            Category::Data => "Data conversion",
            Category::Encoding => "Encoding",
            Category::Security => "Security & identifiers",
            Category::Text => "Text & generators",
            Category::Code => "Code & markup",
        }
    }
}

pub trait Tool {
    /// Shown in the sidebar, the title bar and matched by the search box.
    fn name(&self) -> &'static str;
    /// Icon glyph shown in the sidebar, e.g. `icon::CLOCK` (see `ui::icon`).
    fn icon(&self) -> &'static str;
    fn category(&self) -> Category;
    /// Draws the tool below the title bar. Called every frame while the tool is selected.
    fn ui(&mut self, ui: &mut egui::Ui);
    /// Fills the tool with an example and shows its result. Used by the
    /// "Sample" buttons and by `--sample` on the command line.
    fn load_sample(&mut self) {}
}

/// Turns the input text into output text, or an error message shown in red.
pub type Transform = fn(&str) -> Result<String, String>;

/// A tool that turns its input into output with one or more buttons.
/// The first action is drawn as the primary button.
pub struct TextTool {
    name: &'static str,
    icon: &'static str,
    category: Category,
    description: &'static str,
    actions: Vec<(&'static str, Transform)>,
    /// Example input for the "Sample" button.
    sample: Option<&'static str>,
    /// Highlighting of the output for each action (defaults to the input's).
    output_syntax: Vec<Option<&'static str>>,
    io: TextIo,
}

impl TextTool {
    pub fn new(name: &'static str, icon: &'static str, category: Category, actions: Vec<(&'static str, Transform)>) -> Self {
        let output_syntax = vec![None; actions.len()];
        Self { name, icon, category, description: "Paste or type your input, then choose an action.", actions, sample: None, output_syntax, io: TextIo::default() }
    }

    /// Replaces the default one-line explanation shown under the title.
    pub fn describe(mut self, description: &'static str) -> Self { self.description = description; self }

    /// Adds a "Sample" button that loads `sample` and runs the primary action.
    pub fn sample(mut self, sample: &'static str) -> Self { self.sample = Some(sample); self }

    /// Highlights both panes as `language` (a syntect name or extension: "json", "sql"…).
    pub fn syntax(mut self, language: &'static str) -> Self {
        self.io.input_syntax = Some(language);
        self.output_syntax = vec![Some(language); self.actions.len()];
        self
    }

    /// Highlights the output differently for each action, e.g. `["js", "py", "rs"]`.
    pub fn output_syntax(mut self, languages: &[&'static str]) -> Self {
        self.output_syntax = languages.iter().map(|l| Some(*l)).collect();
        self.output_syntax.resize(self.actions.len(), None);
        self
    }

    /// Shows the output as a "Key: value" report (keys muted, values bright).
    pub fn key_value_output(mut self) -> Self {
        self.output_syntax = vec![Some(ui::KEY_VALUE); self.actions.len()];
        self
    }

    fn run(&mut self, action: usize) {
        self.io.output_syntax = self.output_syntax[action];
        self.io.apply((self.actions[action].1)(&self.io.input));
    }
}

impl Tool for TextTool {
    fn name(&self) -> &'static str { self.name }
    fn icon(&self) -> &'static str { self.icon }
    fn category(&self) -> Category { self.category }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, self.description);
        ui.add_space(10.0);
        let labels: Vec<&str> = self.actions.iter().map(|(label, _)| *label).collect();
        let sample = self.sample;
        ui::split(ui, |left, right| {
            if let Some(i) = self.io.input_pane(left, &labels, "Enter text here…", sample) { self.run(i); }
            self.io.output_pane(right);
        });
    }

    fn load_sample(&mut self) {
        if let Some(sample) = self.sample { self.io.input = sample.to_owned(); }
        self.run(0);
    }
}

/// Every tool, in sidebar order within each category.
pub fn registry() -> Vec<Box<dyn Tool>> {
    vec![
        date::unix_time(),
        data::json_format(),
        encoding::base64_string(),
        Box::new(base64_image::Base64Image::default()),
        security::jwt(),
        Box::new(regex_tester::RegexTester::default()),
        encoding::url(),
        data::url_parser(),
        encoding::html_entity(),
        encoding::backslash(),
        Box::new(identifiers::Identifiers::default()),
        Box::new(diff::Diff::default()),
        code::number_base(),
        format::html(),
        format::css(),
        format::js(),
        format::scss(),
        format::xml(),
        text::lorem_ipsum(),
        Box::new(qr::Qr::default()),
        text::string_inspector(),
        security::hash(),
        Box::new(markdown::Markdown::default()),
        format::sql(),
        text::case_converter(),
        cron::cron(),
        text::color(),
        text::random_string(),
        curl::curl_to_code(),
        json_code::json_to_code(),
        x509::certificate(),
        encoding::hex_to_ascii(),
        encoding::ascii_to_hex(),
        Box::new(lines::Lines::default()),
    ]
}
