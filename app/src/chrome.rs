//! The window chrome: the searchable sidebar and the title bar.
use eframe::egui;
use crate::{palette::Palette, tools::Category, ui::{icon, theme}, App, TITLE_BAR};

/// Seconds a hover highlight takes to fade in or out.
const HOVER_FADE: f32 = 0.12;

impl App {
    /// The resizable left panel: a search box, then the matching tools grouped
    /// by category, with favorites first. Clicking a tool opens it.
    pub(crate) fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools").resizable(true).default_width(260.0).min_width(180.0)
            .frame(egui::Frame::new().fill(theme::SIDEBAR_BG).inner_margin(egui::Margin { left: 12, right: 12, top: 4, bottom: 12 }))
            .show(ctx, |ui| {
                // Empty strip under the traffic lights that still moves the window
                // (with the panel's 4 px top margin, as tall as the title bar).
                let (_, strip) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_BAR - 4.0), egui::Sense::click_and_drag());
                drag_window(ctx, &strip);
                let search = self.search_field(ui);
                if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::F)) { search.request_focus(); }
                ui.add_space(8.0);
                let query = self.search.to_lowercase();
                let visible: Vec<usize> = (0..self.tools.len()).filter(|&i| self.tools[i].name().to_lowercase().contains(&query)).collect();
                let (mut clicked, mut toggle) = (None, None);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    // Favorites first (they also stay in their category), then every category.
                    let favorites: Vec<usize> = self.favorites.iter().copied().filter(|i| visible.contains(i)).collect();
                    let groups = std::iter::once(("Favorites", favorites))
                        .chain(Category::ALL.into_iter().map(|c| (c.label(), visible.iter().copied().filter(|&i| self.tools[i].category() == c).collect())));
                    for (label, members) in groups {
                        if members.is_empty() { continue; }
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(label).small().strong().color(theme::SUBTLE));
                        ui.add_space(3.0);
                        for i in members {
                            let row = self.tool_row(ui, i);
                            if row.clicked() { clicked = Some(i); }
                            // `or`, not `or_else`: every row needs its menu attached.
                            toggle = toggle.or(self.favorite_menu(&row, i));
                        }
                    }
                });
                if let Some(i) = clicked { self.open(ctx, Some(i)); }
                if let Some(i) = toggle { self.toggle_favorite(ctx, i); }
            });
    }

    /// The rounded search box with a magnifying-glass icon.
    fn search_field(&mut self, ui: &mut egui::Ui) -> egui::Response {
        egui::Frame::new()
            .fill(theme::SEARCH_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::SEARCH_STROKE))
            .corner_radius(7).inner_margin(egui::Margin::symmetric(9, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon::MAGNIFYING_GLASS).size(15.0).color(theme::SEARCH_ICON));
                    ui.add(egui::TextEdit::singleline(&mut self.search).frame(false)
                        .hint_text("Search…  ⌘⇧F").desired_width(ui.available_width()))
                }).inner
            }).inner
    }

    /// One clickable sidebar entry: icon and tool name. The hover highlight
    /// fades in and out; the selected row is mint-tinted with an accent bar.
    fn tool_row(&self, ui: &mut egui::Ui, i: usize) -> egui::Response {
        let tool = &self.tools[i];
        let selected = self.selected == Some(i);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
        let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), HOVER_FADE);
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, 5.0, theme::ROW_SELECTED);
            painter.rect_filled(egui::Rect::from_min_size(rect.left_top() + egui::vec2(0.0, 6.0), egui::vec2(3.0, rect.height() - 12.0)), 1.5, theme::ACCENT);
        } else if hover > 0.0 {
            painter.rect_filled(rect, 5.0, theme::ROW_HOVER.gamma_multiply(hover));
        }
        let (icon_color, text_color) = if selected { (theme::ACCENT, theme::TEXT_STRONG) } else { (theme::ICON.lerp_to_gamma(theme::TEXT_STRONG, hover), egui::Color32::LIGHT_GRAY.lerp_to_gamma(theme::TEXT_STRONG, hover)) };
        // The icon nudges right on hover.
        let nudge = 2.0 * hover;
        painter.text(rect.left_center() + egui::vec2(16.0 + nudge, 0.0), egui::Align2::CENTER_CENTER, tool.icon(), egui::FontId::proportional(16.0), icon_color);
        painter.text(rect.left_center() + egui::vec2(32.0 + nudge, 0.0), egui::Align2::LEFT_CENTER, tool.name(), egui::FontId::proportional(13.0), text_color);
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Sidebar toggle and Home button, the current tool's name centered like a
    /// native title bar, and a star for the open tool. The name pill opens the palette.
    pub(crate) fn title_bar(&mut self, ui: &mut egui::Ui) {
        let (bar_rect, bar) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_BAR), egui::Sense::click_and_drag());
        drag_window(ui.ctx(), &bar);
        let (glyph, title) = self.selected.map_or((icon::HOUSE, "DevUtils"), |i| (self.tools[i].icon(), self.tools[i].name()));
        // The pill leaves room for the buttons at both ends.
        let pill = egui::Rect::from_center_size(bar_rect.center(), egui::vec2((bar_rect.width() - 160.0).clamp(80.0, 380.0), 22.0));
        let response = ui.interact(pill, ui.id().with("title_pill"), egui::Sense::click()).on_hover_text("Search tools and commands (⌘K)");
        let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), HOVER_FADE);
        let painter = ui.painter();
        painter.rect_filled(pill, 5.0, theme::PILL.lerp_to_gamma(theme::BUTTON, hover));
        painter.text(pill.center(), egui::Align2::CENTER_CENTER, format!("{glyph}  {title}"), egui::FontId::proportional(12.0), theme::TEXT);
        // The ⌘K hint only fits beside the name in a wide pill.
        if pill.width() > 220.0 {
            painter.text(pill.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, "⌘K", egui::FontId::proportional(11.0), theme::SUBTLE.gamma_multiply(0.5 + 0.5 * hover));
        }
        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { self.palette = Some(Palette::default()); }

        let button_at = |x: f32| egui::Rect::from_min_size(egui::pos2(x, bar_rect.center().y - 13.0), egui::vec2(28.0, 26.0));
        // A star at the right end stars the open tool.
        if let Some(i) = self.selected {
            let starred = self.favorites.contains(&i);
            let tip = if starred { "Remove from favorites (⌘D)" } else { "Add to favorites (⌘D)" };
            let color = if starred { theme::AMBER } else { theme::ICON };
            if bar_button(ui, button_at(bar_rect.right() - 28.0), "title_star", icon::STAR, color, tip).clicked() { self.toggle_favorite(ui.ctx(), i); }
        }
        if bar_button(ui, button_at(bar_rect.left()), "title_sidebar", icon::SIDEBAR_SIMPLE, theme::ICON, "Toggle sidebar (⌘1)").clicked() {
            self.show_sidebar = !self.show_sidebar;
        }
        if bar_button(ui, button_at(bar_rect.left() + 30.0), "title_home", icon::HOUSE, theme::ICON, "Home").clicked() { self.open(ui.ctx(), None); }
    }
}

/// An icon button painted at `rect` of the title bar (which is painted, not laid
/// out): a rounded fill on hover and a tooltip.
fn bar_button(ui: &egui::Ui, rect: egui::Rect, id: &str, glyph: &str, color: egui::Color32, tip: &str) -> egui::Response {
    let response = ui.interact(rect, ui.id().with(id), egui::Sense::click()).on_hover_text(tip);
    if response.hovered() { ui.painter().rect_filled(rect, 5.0, theme::BUTTON_HOVER); }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(17.0), color);
    response
}

/// The title bar is transparent, so let its empty areas move and zoom the window.
fn drag_window(ctx: &egui::Context, response: &egui::Response) {
    if response.drag_started_by(egui::PointerButton::Primary) { ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag); }
    if response.double_clicked() {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
}
