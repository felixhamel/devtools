//! Markdown Editor: Markdown on the left, a live preview or the generated
//! HTML on the right. Tables, task lists, strikethrough and footnotes are
//! supported, and task-list boxes can be ticked in the preview.
use eframe::egui;
use super::{Category, Tool};
use crate::ui::{self, icon, theme, TextIo};

/// Example for the "Sample" button, using most of the syntax.
const SAMPLE: &str = r#"# Release notes

DevUtils is a **native** macOS app with *fast*, ~~slow~~ instant tools.
Read the [README](https://github.com/felixhamel/devtools) for details.

## Checklist

- [x] Write the code
- [ ] Update the screenshots
- [ ] Publish the release

## New tools

| Tool | Category |
|------|----------|
| Date Calculator | Date & scheduling |
| JSON ↔ CSV | Data conversion |
| Markdown Editor | Code & markup |

> Tip: press ⌘K to find any tool.

```rust
fn main() {
    println!("Hello, Markdown!");
}
```

1. Numbered lists
2. Work too[^1]

[^1]: And so do footnotes.
"#;

/// What the right pane shows.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum View {
    #[default]
    Preview,
    Html,
}

/// The Markdown Editor tool: the input is Markdown, the output its HTML.
pub struct Markdown {
    io: TextIo,
    view: View,
    /// State kept between frames by the preview (link hover, code highlighting…).
    cache: egui_commonmark::CommonMarkCache,
}

impl Default for Markdown {
    fn default() -> Self {
        let io = TextIo { input_syntax: Some("md"), output_syntax: Some("html"), ..TextIo::default() };
        Self { io, view: View::Preview, cache: egui_commonmark::CommonMarkCache::default() }
    }
}

/// The HTML for `markdown`, with the extensions the preview shows.
fn to_html(markdown: &str) -> String {
    use pulldown_cmark::Options;
    let options = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES;
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new_ext(markdown, options));
    html
}

impl Markdown {
    /// Updates the HTML from the Markdown.
    fn run(&mut self) {
        self.io.output = to_html(&self.io.input);
    }
}

impl Tool for Markdown {
    fn name(&self) -> &'static str { "Markdown Editor" }
    fn icon(&self) -> &'static str { icon::MARKDOWN_LOGO }
    fn category(&self) -> Category { Category::Code }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // The ⇄ menu leaves out the tool it is drawn in.
        let tool_name = self.name();
        ui::description(ui, "Write Markdown with a live preview, then copy the HTML. Tick the task-list boxes right in the preview.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            if self.io.input_pane(left, &[], "Write Markdown here…", Some(SAMPLE)) { self.run(); }
            let reuse = ui::toolbar(right, "Output:", |ui| {
                let (preview, html) = (format!("{}  Preview", icon::EYE), format!("{}  HTML", icon::CODE));
                ui::segmented(ui, &mut self.view, &[(View::Preview, preview.as_str()), (View::Html, html.as_str())]);
                ui.separator();
                ui::copy_button(ui, &self.io.output);
                super::reuse_menu(ui, &self.io.output, tool_name)
            });
            // HTML is valid Markdown, so "Use as input here" shows the same document.
            if reuse { self.io.input = std::mem::take(&mut self.io.output); self.run(); }
            match self.view {
                View::Preview => ui::preview_frame(right, |ui| {
                    if self.io.input.trim().is_empty() {
                        ui.label(egui::RichText::new("The preview of your Markdown appears here.").color(theme::SUBTLE));
                        return;
                    }
                    // A ticked box rewrites the Markdown: compare only on clicks, when it can happen.
                    let before = ui.input(|i| i.pointer.any_click()).then(|| self.io.input.clone());
                    egui_commonmark::CommonMarkViewer::new().show_mut(ui, &mut self.cache, &mut self.io.input);
                    if before.is_some_and(|text| text != self.io.input) { self.run(); }
                }),
                View::Html => { ui::code_editor(right, "html", &mut self.io.output, "", ui::Highlight::Syntax("html")); }
            }
        });
    }

    fn load_sample(&mut self) {
        self.io.input = SAMPLE.into();
        self.run();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.io.input = text.to_owned();
        self.run();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_html_with_extensions() {
        let html = to_html(SAMPLE);
        for part in ["<h1>Release notes</h1>", "<strong>native</strong>", "<del>slow</del>", "<input disabled=\"\" type=\"checkbox\" checked=\"\"/>",
            "<table>", "<td>Date Calculator</td>", "<blockquote>", "<code class=\"language-rust\">", "class=\"footnote-definition\""] {
            assert!(html.contains(part), "{part} missing in\n{html}");
        }
        assert_eq!(to_html(""), "");
    }
}
