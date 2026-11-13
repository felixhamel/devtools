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
    /// Short text (2–3 characters) shown in the sidebar badge.
    fn badge(&self) -> &'static str;
    fn category(&self) -> Category;
    /// Draws the tool below the title bar. Called every frame while the tool is selected.
    fn ui(&mut self, ui: &mut egui::Ui);
}

/// Turns the input text into output text, or an error message shown in red.
pub type Transform = fn(&str) -> Result<String, String>;

/// A tool that turns its input into output with one or more buttons.
/// The first action is drawn as the primary button.
pub struct TextTool {
    name: &'static str,
    badge: &'static str,
    category: Category,
    description: &'static str,
    actions: Vec<(&'static str, Transform)>,
    io: TextIo,
}

impl TextTool {
    pub fn new(name: &'static str, badge: &'static str, category: Category, actions: Vec<(&'static str, Transform)>) -> Self {
        Self { name, badge, category, description: "Paste or type your input, then choose an action.", actions, io: TextIo::default() }
    }

    /// Replaces the default one-line explanation shown under the title.
    pub fn describe(mut self, description: &'static str) -> Self { self.description = description; self }
}

impl Tool for TextTool {
    fn name(&self) -> &'static str { self.name }
    fn badge(&self) -> &'static str { self.badge }
    fn category(&self) -> Category { self.category }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, self.description);
        ui.add_space(10.0);
        let labels: Vec<&str> = self.actions.iter().map(|(label, _)| *label).collect();
        ui::split(ui, |left, right| {
            if let Some(i) = self.io.input_pane(left, &labels, "Enter text here…") { self.io.apply((self.actions[i].1)(&self.io.input)); }
            self.io.output_pane(right);
        });
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
