//! The home screen: the paste-anything card, favorites, recent tools and
//! every tool as a card; plus what ⌘V and dropped files do there.
use eframe::egui;
use crate::{palette::Palette, tools::{self, Category, Tool}, ui::{self, icon, theme}, App};

/// The home screen's content is centered and at most this wide.
const CONTENT_MAX_WIDTH: f32 = 900.0;
/// Tool cards are at least this wide; the grid has as many columns as fit (at least 2).
const CARD_MIN_WIDTH: f32 = 210.0;
/// Space between tool cards, both ways.
const CARD_GAP: f32 = 10.0;

impl App {
    /// Opens `text` in the tool that recognizes it (a JWT in the JWT
    /// Debugger…), or in the String Inspector when nothing matches.
    pub(crate) fn open_text(&mut self, ctx: &egui::Context, text: &str) {
        let (index, message) = match tools::detect::detect(text) {
            Some(found) => (self.tool_index(found.tool), format!("Looks like {} — opened {}", found.what, found.tool)),
            None => (self.tool_index("String Inspector"), "No known format — opened the String Inspector".to_owned()),
        };
        let Some(i) = index else { return };
        self.tools[i].set_input(text.trim());
        self.open(ctx, Some(i));
        ui::toast(ctx, icon::MAGIC_WAND, message);
    }

    /// On the home screen: ⌘V opens the pasted text in the right tool, and
    /// dropped files open in the tool that suits them. Runs before the tools
    /// are drawn, so a tool selected here still sees this frame's dropped files.
    pub(crate) fn home_input(&mut self, ctx: &egui::Context) {
        let typing = ctx.memory(|m| m.focused().is_some());
        let pasted = ctx.input(|i| i.events.iter().find_map(|e| match e { egui::Event::Paste(text) => Some(text.clone()), _ => None }));
        if let (Some(text), false) = (pasted, typing) {
            self.open_text(ctx, &text);
            return;
        }
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        if files.is_empty() { return; }
        let is_image = |f: &egui::DroppedFile| {
            let name = f.path.as_ref().map_or(f.name.clone(), |p| p.display().to_string()).to_lowercase();
            ui::IMAGE_EXTENSIONS.iter().any(|ext| name.ends_with(&format!(".{ext}")))
        };
        let images = files.iter().filter(|f| is_image(f)).count();
        let (target, message) = match (files.len(), images) {
            (1, 1) => ("Base64 Image Encode/Decode", "Image encoded in Base64"),
            (_, n) if n >= 2 => ("Image Compare", "Images opened in Image Compare"),
            (1, _) => {
                // A text file whose content is recognized opens like a paste; others go to Code Compare.
                let text = ui::read_dropped(&files[0]).ok().and_then(|(_, bytes)| String::from_utf8(bytes).ok());
                if let Some(text) = text.filter(|t| tools::detect::detect(t).is_some()) {
                    ctx.input_mut(|i| i.raw.dropped_files.clear());
                    self.open_text(ctx, &text);
                    return;
                }
                ("Code Compare", "Opened in Code Compare — drop a second file to compare")
            }
            _ => ("Code Compare", "Files opened in Code Compare"),
        };
        if let Some(i) = self.tool_index(target) {
            self.open(ctx, Some(i));
            ui::toast(ctx, icon::DOWNLOAD_SIMPLE, message);
        }
    }

    /// The home screen: logo, the paste-or-drop card, favorites, recent tools
    /// and every tool as a card. Right-clicking a card stars or unstars it.
    pub(crate) fn home(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let area = ui.max_rect();
        let (mut clicked, mut toggle) = (None, None);
        let mut paste = false;
        let mut palette = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let width = (ui.available_width() - 8.0).min(CONTENT_MAX_WIDTH);
            let margin = ((ui.available_width() - width) / 2.0).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(margin);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.add_space(14.0);
                    ui.vertical_centered(|ui| {
                        // The logo grows a little under the pointer.
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
                        let grow = ctx.animate_bool_with_time(response.id, response.hovered(), 0.2);
                        let rect = egui::Rect::from_center_size(rect.center(), rect.size() * (1.0 + 0.08 * grow));
                        egui::Image::new(&self.logo).paint_at(ui, rect);
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("DevUtils").size(24.0).strong().color(theme::TEXT_STRONG));
                        ui.label(egui::RichText::new(format!("{} everyday developer tools, all offline.", self.tools.len())).color(theme::MUTED));
                    });
                    ui.add_space(18.0);
                    (paste, palette) = smart_card(ui);
                    let columns = ((width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)).floor().max(2.0) as usize;
                    let card_width = (width - CARD_GAP * (columns - 1) as f32) / columns as f32;
                    // A titled grid of tool cards; reports a click and a favorite toggle.
                    let section = |ui: &mut egui::Ui, label: &str, color: egui::Color32, members: &[usize], clicked: &mut Option<usize>, toggle: &mut Option<usize>| {
                        ui.add_space(18.0);
                        section_title(ui, label, color);
                        for row in members.chunks(columns) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = CARD_GAP;
                                for &i in row {
                                    let card = tool_card(ui, self.tools[i].as_ref(), card_width);
                                    if card.clicked() { *clicked = Some(i); }
                                    // `or`, not `or_else`: every card needs its menu attached.
                                    *toggle = toggle.or(self.favorite_menu(&card, i));
                                }
                            });
                            ui.add_space(CARD_GAP - ui.spacing().item_spacing.y);
                        }
                    };
                    // Favorites, then recent tools, then every tool grouped like the sidebar.
                    if !self.favorites.is_empty() {
                        section(ui, "Favorites", theme::AMBER, &self.favorites, &mut clicked, &mut toggle);
                    }
                    if !self.recent.is_empty() {
                        ui.add_space(18.0);
                        section_title(ui, "Recent", theme::TEXT);
                        ui.horizontal_wrapped(|ui| {
                            for &i in &self.recent {
                                let tool = &self.tools[i];
                                let text = egui::RichText::new(format!("{}  {}", tool.icon(), tool.name())).color(theme::TEXT);
                                let chip = egui::Button::new(text).fill(theme::CARD).stroke(egui::Stroke::new(1.0_f32, theme::EDITOR_STROKE)).corner_radius(14);
                                if ui.add(chip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { clicked = Some(i); }
                            }
                        });
                    }
                    for category in Category::ALL {
                        let members: Vec<usize> = (0..self.tools.len()).filter(|&i| self.tools[i].category() == category).collect();
                        section(ui, category.label(), category.color(), &members, &mut clicked, &mut toggle);
                    }
                    ui.add_space(24.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("⌘K search  ·  ⌘V paste anything  ·  ⌘D star a tool  ·  ⌘1 sidebar  ·  ⌘⇧F filter the sidebar").small().color(theme::SUBTLE));
                    });
                    ui.add_space(12.0);
                });
            });
        });
        if let Some(i) = clicked { self.open(&ctx, Some(i)); }
        if let Some(i) = toggle { self.toggle_favorite(&ctx, i); }
        if palette { self.palette = Some(Palette::default()); }
        if paste {
            match ui::clipboard_text() {
                Ok(text) => self.open_text(&ctx, &text),
                Err(e) => ui::toast(&ctx, icon::WARNING, e),
            }
        }
        ui::drop_hint(&ctx, area, "Drop files: one to open, two to compare");
    }
}

/// A small colored section heading on the home screen.
fn section_title(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(dot.center(), 3.5, color);
        ui.label(egui::RichText::new(text).strong().color(theme::TEXT));
    });
    ui.add_space(4.0);
}

/// The "paste anything / drop files" card of the home screen. Returns
/// whether its Paste and Search buttons were clicked.
fn smart_card(ui: &mut egui::Ui) -> (bool, bool) {
    let (mut paste, mut search) = (false, false);
    egui::Frame::new().fill(theme::ROW_SELECTED.gamma_multiply(0.7)).stroke(egui::Stroke::new(1.0_f32, theme::ACCENT_DARK))
        .corner_radius(12).inner_margin(egui::Margin::symmetric(18, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon::MAGIC_WAND).size(30.0).color(theme::ACCENT));
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Paste anything with ⌘V").size(15.0).strong().color(theme::TEXT_STRONG));
                    ui.label(egui::RichText::new("JWTs, JSON, timestamps, UUIDs, URLs, colors, cron, certificates… open in the right tool.").color(theme::MUTED));
                    ui.label(egui::RichText::new("Or drop files here: two images or two texts open side by side in Compare.").color(theme::MUTED));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    search = ui.add(ui::icon_button(icon::MAGNIFYING_GLASS, "Search  ⌘K", false)).clicked();
                    paste = ui.add(ui::icon_button(icon::CLIPBOARD_TEXT, "Paste", true)).clicked();
                });
            });
        });
    (paste, search)
}

/// A tool card of the home screen: it lifts and takes its category's color under the pointer.
fn tool_card(ui: &mut egui::Ui, tool: &dyn Tool, width: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 48.0), egui::Sense::click());
    let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.15);
    let color = tool.category().color();
    let card = rect.translate(egui::vec2(0.0, -2.0 * hover));
    let painter = ui.painter();
    if hover > 0.0 { painter.rect_filled(card.translate(egui::vec2(0.0, 4.0)), 10.0, egui::Color32::from_black_alpha((70.0 * hover) as u8)); }
    let stroke = theme::EDITOR_STROKE.lerp_to_gamma(color.gamma_multiply(0.8), hover);
    painter.rect(card, 10.0, theme::CARD.lerp_to_gamma(theme::CARD_HOVER, hover), egui::Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
    let center = card.left_center() + egui::vec2(25.0, 0.0);
    painter.circle_filled(center, 15.0, color.gamma_multiply(0.16 + 0.12 * hover));
    painter.text(center, egui::Align2::CENTER_CENTER, tool.icon(), egui::FontId::proportional(16.0), color);
    let mut name = egui::text::LayoutJob::simple_singleline(tool.name().to_owned(), egui::FontId::proportional(13.0), theme::TEXT_STRONG);
    name.wrap = egui::text::TextWrapping { max_width: width - 60.0, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let galley = painter.layout_job(name);
    painter.galley(card.left_center() + egui::vec2(50.0, -galley.size().y / 2.0), galley, theme::TEXT_STRONG);
    response.on_hover_text(tool.category().label()).on_hover_cursor(egui::CursorIcon::PointingHand)
}
