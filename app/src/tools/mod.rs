//! Tool registry. To add a tool: implement `Tool` (or build a `TextTool`)
//! and add it to `registry()`.
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
    fn name(&self) -> &'static str;
    /// Short text shown in the sidebar badge.
    fn badge(&self) -> &'static str;
    fn category(&self) -> Category;
    fn ui(&mut self, ui: &mut egui::Ui);
}

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
