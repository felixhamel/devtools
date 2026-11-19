//! The ⌘K command palette: fuzzy search over the tools and a few commands.
use eframe::egui;
use crate::{tools::Category, ui::{self, icon, theme}, App};

/// The ⌘K palette: type to find a tool or a command, ↑↓ to choose, ↵ to run.
#[derive(Default)]
pub(crate) struct Palette {
    query: String,
    /// Index of the highlighted row in the current entries.
    selected: usize,
}

/// What a palette row does.
#[derive(Clone, Copy, PartialEq)]
enum Command {
    /// Open the tool at this index.
    Tool(usize),
    /// Go back to the home screen.
    Home,
    ToggleSidebar,
    /// Open the clipboard's text in the tool that recognizes it.
    PasteAndDetect,
    /// Star or unstar the tool at this index (offered for the open tool).
    ToggleFavorite(usize),
}

/// How well `query` matches `text`, higher is better: a substring scores
/// best (more so at a word start and early on), then the query's characters
/// in order with few gaps. `None` when it does not match at all.
fn fuzzy_score(text: &str, query: &str) -> Option<i32> {
    let (text, query) = (text.to_lowercase(), query.to_lowercase());
    // Substrings score about 1000 (+500 at a word start), above any
    // scattered match (at most 500).
    if let Some(at) = text.find(&query) {
        let word_start = text[..at].chars().last().is_none_or(|c| !c.is_alphanumeric());
        return Some(1000 - at as i32 + if word_start { 500 } else { 0 });
    }
    // Scattered: each query character found further on in `text`, in order.
    let mut chars = text.chars();
    let mut gaps = 0;
    for q in query.chars() {
        let mut found = false;
        for c in chars.by_ref() {
            if c == q { found = true; break; }
            gaps += 1;
        }
        if !found { return None; }
    }
    Some(500 - gaps.min(499))
}

impl App {
    /// Palette rows for `query`: best matches first. With no query:
    /// favorites, recent tools, the commands, then every tool in sidebar order.
    fn palette_entries(&self, query: &str) -> Vec<Command> {
        let star = self.selected.map(Command::ToggleFavorite);
        let commands: Vec<Command> = [Command::PasteAndDetect, Command::Home, Command::ToggleSidebar].into_iter().chain(star).collect();
        let by_category = Category::ALL.into_iter().flat_map(|c| (0..self.tools.len()).filter(move |&i| self.tools[i].category() == c));
        let query = query.trim();
        if query.is_empty() {
            let mut first: Vec<usize> = self.favorites.clone();
            first.extend(self.recent.iter().filter(|i| !self.favorites.contains(i)));
            let others: Vec<usize> = by_category.filter(|i| !first.contains(i)).collect();
            return first.into_iter().map(Command::Tool).chain(commands).chain(others.into_iter().map(Command::Tool)).collect();
        }
        let mut scored: Vec<(i32, Command)> = by_category.map(Command::Tool).chain(commands).filter_map(|command| {
            let (_, name, detail, _) = self.describe(command);
            // A match in the detail (category, shortcut) ranks well below one in the name.
            let score = fuzzy_score(&name, query).max(fuzzy_score(&detail, query).map(|s| s - 400))?;
            Some((score, command))
        }).collect();
        scored.sort_by_key(|(score, _)| -score);
        scored.into_iter().map(|(_, command)| command).collect()
    }

    /// Icon, name, detail and color of a palette row.
    fn describe(&self, command: Command) -> (&'static str, String, String, egui::Color32) {
        match command {
            Command::Tool(i) => {
                let tool = &self.tools[i];
                let detail = if self.favorites.contains(&i) { format!("★ {}", tool.category().label()) } else { tool.category().label().to_owned() };
                (tool.icon(), tool.name().to_owned(), detail, tool.category().color())
            }
            Command::ToggleFavorite(i) => {
                let action = if self.favorites.contains(&i) { "Remove from favorites" } else { "Add to favorites" };
                (icon::STAR, format!("{action}: {}", self.tools[i].name()), "⌘D".into(), theme::AMBER)
            }
            Command::PasteAndDetect => (icon::MAGIC_WAND, "Paste and detect".into(), "Open the clipboard in the right tool".into(), theme::ACCENT),
            Command::Home => (icon::HOUSE, "Home".into(), "Back to the home screen".into(), theme::ACCENT),
            Command::ToggleSidebar => (icon::SIDEBAR_SIMPLE, "Toggle sidebar".into(), "⌘1".into(), theme::ACCENT),
        }
    }

    /// Does what the chosen row says.
    fn run(&mut self, ctx: &egui::Context, command: Command) {
        match command {
            Command::Tool(i) => self.open(ctx, Some(i)),
            Command::Home => self.open(ctx, None),
            Command::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Command::ToggleFavorite(i) => self.toggle_favorite(ctx, i),
            Command::PasteAndDetect => match ui::clipboard_text() {
                Ok(text) => self.open_text(ctx, &text),
                Err(e) => ui::toast(ctx, icon::WARNING, e),
            },
        }
    }

    /// Draws the ⌘K palette when it is open, and runs the chosen command.
    pub(crate) fn palette(&mut self, ctx: &egui::Context) {
        let Some(mut palette) = self.palette.take() else { return };
        // Navigation keys are taken before the search field can see them.
        let (up, down, enter, escape) = ctx.input_mut(|i| (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        ));
        let entries = self.palette_entries(&palette.query);
        let count = entries.len().max(1);
        if up { palette.selected = (palette.selected + count - 1) % count; }
        if down { palette.selected = (palette.selected + 1) % count; }
        palette.selected = palette.selected.min(entries.len().saturating_sub(1));
        let mut chosen = None;
        let id = egui::Id::new("command-palette");
        let frame = egui::Frame::new().fill(theme::PALETTE_BG).stroke(egui::Stroke::new(1.0_f32, theme::SEARCH_STROKE))
            .corner_radius(12).inner_margin(10)
            .shadow(egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: egui::Color32::from_black_alpha(140) });
        let modal = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 90.0)))
            .backdrop_color(egui::Color32::from_black_alpha(110)).frame(frame)
            .show(ctx, |ui| {
                ui.set_width(560.0_f32.min(ctx.screen_rect().width() - 40.0));
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon::MAGNIFYING_GLASS).size(18.0).color(theme::ACCENT));
                    let field = ui.add(egui::TextEdit::singleline(&mut palette.query).frame(false).font(egui::FontId::proportional(17.0))
                        .hint_text("Search tools and commands…").desired_width(f32::INFINITY));
                    field.request_focus();
                    if field.changed() { palette.selected = 0; }
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(2.0);
                // Hovering selects a row only when the pointer moves, so a row
                // under a still pointer does not undo the arrow keys.
                let mouse_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
                egui::ScrollArea::vertical().max_height(380.0).auto_shrink([false, true]).show(ui, |ui| {
                    if entries.is_empty() {
                        ui.add_space(10.0);
                        ui.vertical_centered(|ui| ui.label(egui::RichText::new("No tool or command matches").color(theme::SUBTLE)));
                        ui.add_space(10.0);
                    }
                    for (n, &command) in entries.iter().enumerate() {
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), egui::Sense::click());
                        if response.hovered() && mouse_moved { palette.selected = n; }
                        let selected = n == palette.selected;
                        if selected && (up || down) { response.scroll_to_me(None); }
                        let (glyph, name, detail, color) = self.describe(command);
                        let painter = ui.painter();
                        if selected { painter.rect_filled(rect, 7.0, theme::ROW_SELECTED); }
                        let center = rect.left_center() + egui::vec2(20.0, 0.0);
                        painter.circle_filled(center, 13.0, color.gamma_multiply(if selected { 0.28 } else { 0.16 }));
                        painter.text(center, egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(15.0), color);
                        painter.text(rect.left_center() + egui::vec2(42.0, 0.0), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(14.0),
                            if selected { theme::TEXT_STRONG } else { theme::TEXT });
                        let detail_color = if selected { theme::MUTED } else { theme::SUBTLE };
                        painter.text(rect.right_center() - egui::vec2(12.0, 0.0), egui::Align2::RIGHT_CENTER, detail, egui::FontId::proportional(12.0), detail_color);
                        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { chosen = Some(command); }
                    }
                });
                ui.add_space(4.0);
                ui.separator();
                ui.horizontal(|ui| {
                    for (keys, action) in [("↑↓", "navigate"), ("return", "open"), ("esc", "close")] {
                        ui.label(egui::RichText::new(keys).small().strong().color(theme::MUTED));
                        ui.label(egui::RichText::new(action).small().color(theme::SUBTLE));
                        ui.add_space(8.0);
                    }
                });
            });
        if enter { chosen = chosen.or_else(|| entries.get(palette.selected).copied()); }
        if let Some(command) = chosen {
            self.run(ctx, command);
        } else if !(escape || modal.backdrop_response.clicked()) {
            self.palette = Some(palette);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matching_prefers_substrings_at_word_starts() {
        assert!(fuzzy_score("JSON Format/Validate", "json").unwrap() > fuzzy_score("JSON Format/Validate", "form").unwrap());
        assert!(fuzzy_score("Image Compare", "comp").unwrap() > fuzzy_score("Image Compare", "mgcp").unwrap());
        assert!(fuzzy_score("Base64 Image Encode/Decode", "b64 img").is_some());
        assert_eq!(fuzzy_score("Hash Generator", "xyz"), None);
    }
}
