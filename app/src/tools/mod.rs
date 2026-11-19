//! The tool registry.
//!
//! Adding a tool:
//! - text in, text out: build a [`TextTool`] with one or more transform functions;
//! - anything else: implement [`Tool`] on a struct that holds the tool's state.
//!
//! Then add it to [`registry()`]. Each tool keeps its own state while the app runs,
//! so switching tools does not lose what was typed.
use eframe::egui;
use crate::ui::{self, icon, TextIo};

mod base64_image;
mod base64_string;
mod case;
mod code;
mod code_compare;
mod color;
mod cron;
pub mod detect;
mod encoding;
mod format;
mod identifiers;
mod image_compare;
mod inspector;
mod json_format;
mod jwt;
mod lines;
mod password;
mod qr;
mod regex_tester;
mod security;
mod text;
mod unix_time;
mod url_parser;
mod x509;

/// Sidebar groups, shown in the order of [`Category::ALL`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category { Date, Data, Encoding, Security, Text, Compare, Code }

impl Category {
    pub const ALL: [Category; 7] = [Category::Date, Category::Data, Category::Encoding, Category::Security, Category::Text, Category::Compare, Category::Code];

    pub fn label(self) -> &'static str {
        match self {
            Category::Date => "Date & scheduling",
            Category::Data => "Data conversion",
            Category::Encoding => "Encoding",
            Category::Security => "Security & identifiers",
            Category::Text => "Text & generators",
            Category::Compare => "Compare",
            Category::Code => "Code & markup",
        }
    }

    /// The category's color, used for tool icons on the home screen and in the palette.
    pub fn color(self) -> egui::Color32 {
        use ui::theme;
        match self {
            Category::Date => theme::AMBER,
            Category::Data => theme::BLUE,
            Category::Encoding => theme::VIOLET,
            Category::Security => theme::ROSE,
            Category::Text => theme::ACCENT,
            Category::Compare => theme::PINK,
            Category::Code => theme::CYAN,
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
    /// Whether [`Tool::set_input`] does something, i.e. other tools can send
    /// their output here with the ⇄ menu.
    fn accepts_input(&self) -> bool { false }
    /// Replaces the tool's main input with `text` and updates the result.
    fn set_input(&mut self, _text: &str) {}
}

/// A tool that can receive text, as listed in the ⇄ menu.
#[derive(Clone)]
struct Target { name: &'static str, icon: &'static str, category: Category }

/// The tools that accept input, stored in egui's memory at startup so that any
/// tool can draw the ⇄ menu without knowing about the others.
#[derive(Clone, Default)]
struct Directory(Vec<Target>);

/// "Send this text to that tool", picked in a ⇄ menu and carried out by the app.
#[derive(Clone)]
pub struct SendRequest { pub target: &'static str, pub text: String }

fn directory_id() -> egui::Id { egui::Id::new("devutils-tool-directory") }
fn request_id() -> egui::Id { egui::Id::new("devutils-send-request") }

/// Makes the list of tools available to the ⇄ menus.
pub fn publish_directory(ctx: &egui::Context, tools: &[Box<dyn Tool>]) {
    let targets = tools.iter().filter(|t| t.accepts_input())
        .map(|t| Target { name: t.name(), icon: t.icon(), category: t.category() }).collect();
    ctx.data_mut(|d| d.insert_temp(directory_id(), Directory(targets)));
}

/// The pending "send to tool" request, if a ⇄ menu item was clicked this frame.
pub fn take_send_request(ctx: &egui::Context) -> Option<SendRequest> {
    ctx.data_mut(|d| {
        let request = d.get_temp::<SendRequest>(request_id());
        if request.is_some() { d.remove::<SendRequest>(request_id()); }
        request
    })
}

/// The ⇄ menu next to an output: "Use as input here" (returns true) or send
/// `text` to another tool, grouped by category. `current` is left out.
pub fn reuse_menu(ui: &mut egui::Ui, text: &str, current: &str) -> bool {
    let mut use_here = false;
    ui.add_enabled_ui(!text.is_empty(), |ui| {
        ui.menu_button(format!("{}  Reuse", icon::ARROWS_LEFT_RIGHT), |ui| {
            if ui.button(format!("{}  Use as input here", icon::ARROW_U_DOWN_LEFT)).clicked() {
                use_here = true;
                ui.close_menu();
            }
            ui.separator();
            ui.label(egui::RichText::new("Send to").small().color(ui::theme::SUBTLE));
            let directory = ui.data(|d| d.get_temp::<Directory>(directory_id())).unwrap_or_default();
            for category in Category::ALL {
                let targets: Vec<&Target> = directory.0.iter().filter(|t| t.category == category && t.name != current).collect();
                if targets.is_empty() { continue; }
                ui.menu_button(category.label(), |ui| {
                    for target in targets {
                        if ui.button(format!("{}  {}", target.icon, target.name)).clicked() {
                            let request = SendRequest { target: target.name, text: text.to_owned() };
                            ui.data_mut(|d| d.insert_temp(request_id(), request));
                            ui.close_menu();
                        }
                    }
                });
            }
        }).response.on_hover_text("Use this result as input, here or in another tool");
    });
    use_here
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

    /// Shows the output as a "Key: value" report (keys muted, values bright).
    pub fn key_value_output(mut self) -> Self {
        self.output_syntax = vec![Some(ui::KEY_VALUE); self.actions.len()];
        self
    }

    /// Runs the selected action on the input. An empty input simply clears the result.
    fn run(&mut self) {
        let action = self.io.active;
        self.io.output_syntax = self.output_syntax[action];
        if self.io.input.trim().is_empty() {
            self.io.output.clear();
            self.io.error.clear();
        } else {
            self.io.apply((self.actions[action].1)(&self.io.input));
        }
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
        let (sample, name) = (self.sample, self.name);
        ui::split(ui, |left, right| {
            if self.io.input_pane(left, &labels, "Enter text here…", sample) { self.run(); }
            if self.io.output_pane(right, |ui, output| reuse_menu(ui, output, name)) {
                self.io.input = std::mem::take(&mut self.io.output);
                self.run();
            }
        });
    }

    fn load_sample(&mut self) {
        if let Some(sample) = self.sample { self.io.input = sample.to_owned(); }
        self.io.active = 0;
        self.run();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.io.input = text.to_owned();
        self.run();
    }
}

/// Every tool, in sidebar order within each category.
pub fn registry() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(unix_time::UnixTime::default()),
        Box::new(json_format::JsonFormat::default()),
        Box::new(base64_string::Base64String::default()),
        Box::new(base64_image::Base64Image::default()),
        Box::new(jwt::Jwt::default()),
        Box::new(regex_tester::RegexTester::default()),
        encoding::url(),
        Box::new(url_parser::UrlParser::default()),
        encoding::html_entity(),
        encoding::backslash(),
        Box::new(identifiers::Identifiers::default()),
        Box::new(code_compare::CodeCompare::default()),
        Box::new(image_compare::ImageCompare::default()),
        code::number_base(),
        format::html(),
        format::css(),
        format::js(),
        format::xml(),
        Box::new(qr::Qr::default()),
        Box::new(inspector::StringInspector::default()),
        security::hash(),
        format::sql(),
        Box::new(case::CaseConverter::default()),
        cron::cron(),
        Box::new(color::ColorTool::default()),
        Box::new(password::Password::default()),
        x509::certificate(),
        encoding::hex(),
        Box::new(lines::Lines::default()),
    ]
}
