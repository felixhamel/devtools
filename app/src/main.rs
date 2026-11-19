//! DevUtils: a native collection of small developer tools, built with egui.
//!
//! `main` opens the window; `App` draws the sidebar (searchable tool list),
//! the title bar, the home screen, the ⌘K palette and the selected tool.
//! Tools live in `tools/`, shared widgets and colors in `ui.rs`.
use eframe::egui;

mod tools;
mod ui;

use tools::{Category, Tool};
use ui::{icon, theme};

/// Height of the transparent title bar area (the traffic-light buttons sit in it on macOS).
const TITLE_BAR: f32 = 30.0;
/// Width reserved for the traffic-light buttons when the sidebar is hidden.
const TRAFFIC_LIGHTS: f32 = 70.0;
const ICON_PNG: &[u8] = include_bytes!("../assets/app-icon.png");
/// Seconds a newly opened tool takes to fade in.
const FADE_IN: f64 = 0.18;
/// Image files that open in the image tools when dropped on the home screen.
const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "webp", "bmp", "gif"];

struct App {
    tools: Vec<Box<dyn Tool>>,
    /// Index into `tools`; `None` shows the home screen.
    selected: Option<usize>,
    /// Recently opened tools, most recent first.
    recent: Vec<usize>,
    /// When the current tool was opened (`ctx` time), for the fade-in.
    opened_at: f64,
    search: String,
    show_sidebar: bool,
    palette: Option<Palette>,
    logo: egui::TextureHandle,
    screenshot: Option<Screenshot>,
}

/// The ⌘K palette: type to find a tool or a command, ↑↓ to choose, ↵ to run.
#[derive(Default)]
struct Palette { query: String, selected: usize }

/// What a palette row does.
#[derive(Clone, Copy, PartialEq)]
enum Command { Tool(usize), Home, ToggleSidebar, PasteAndDetect }

/// `--screenshot <path>` mode: render, save a PNG of the window and exit.
/// Used to check that a build starts and draws.
struct Screenshot { path: String, frames: u32 }

/// Command-line options, mostly for checking the interface:
/// `--tool <name>` opens a tool (name matched case-insensitively),
/// `--sample` loads its example, `--palette` opens the ⌘K palette,
/// `--screenshot <path>` captures the window and quits.
#[derive(Default)]
struct Args { screenshot: Option<String>, tool: Option<String>, sample: bool, palette: bool }

impl Args {
    fn parse() -> Self {
        let mut args = Args::default();
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--screenshot" => args.screenshot = it.next(),
                "--tool" => args.tool = it.next(),
                "--sample" => args.sample = true,
                "--palette" => args.palette = true,
                _ => {}
            }
        }
        args
    }
}

/// How well `query` matches `text`, higher is better: a substring scores
/// best (more so at a word start and early on), then the query's characters
/// in order with few gaps. `None` when it does not match at all.
fn fuzzy_score(text: &str, query: &str) -> Option<i32> {
    let (text, query) = (text.to_lowercase(), query.to_lowercase());
    if let Some(at) = text.find(&query) {
        let word_start = text[..at].chars().last().is_none_or(|c| !c.is_alphanumeric());
        return Some(1000 - at as i32 + if word_start { 500 } else { 0 });
    }
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
    fn new(cc: &eframe::CreationContext, icon: &image::RgbaImage, args: Args) -> Self {
        ui::apply_visuals(&cc.egui_ctx);
        ui::install_fonts(&cc.egui_ctx);
        let logo = image::imageops::resize(icon, 144, 144, image::imageops::FilterType::Lanczos3);
        let logo = egui::ColorImage::from_rgba_unmultiplied([logo.width() as usize, logo.height() as usize], logo.as_raw());
        let mut tools = tools::registry();
        tools::publish_directory(&cc.egui_ctx, &tools);
        let selected = args.tool.and_then(|wanted| tools.iter().position(|t| t.name().eq_ignore_ascii_case(&wanted)));
        if let (Some(i), true) = (selected, args.sample) { tools[i].load_sample(); }
        Self {
            tools,
            selected,
            recent: selected.into_iter().collect(),
            opened_at: f64::NEG_INFINITY,
            search: String::new(),
            show_sidebar: true,
            palette: args.palette.then(Palette::default),
            logo: cc.egui_ctx.load_texture("logo", logo, Default::default()),
            screenshot: args.screenshot.map(|path| Screenshot { path, frames: 0 }),
        }
    }

    fn tool_index(&self, name: &str) -> Option<usize> { self.tools.iter().position(|t| t.name() == name) }

    /// Shows tool `index` (or the home screen for `None`) and remembers it as recent.
    fn open(&mut self, ctx: &egui::Context, index: Option<usize>) {
        if self.selected != index { self.opened_at = ctx.input(|i| i.time); }
        self.selected = index;
        if let Some(i) = index {
            self.recent.retain(|&r| r != i);
            self.recent.insert(0, i);
            self.recent.truncate(6);
        }
    }

    /// Opens `text` in the tool that recognizes it (a JWT in the JWT
    /// Debugger…), or in the String Inspector when nothing matches.
    fn open_text(&mut self, ctx: &egui::Context, text: &str) {
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
    fn home_input(&mut self, ctx: &egui::Context) {
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
            IMAGE_EXTENSIONS.iter().any(|ext| name.ends_with(&format!(".{ext}")))
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

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools").resizable(true).default_width(260.0).min_width(180.0)
            .frame(egui::Frame::new().fill(theme::SIDEBAR_BG).inner_margin(egui::Margin { left: 12, right: 12, top: 4, bottom: 12 }))
            .show(ctx, |ui| {
                // Empty strip under the traffic lights that still moves the window.
                let (_, strip) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_BAR - 4.0), egui::Sense::click_and_drag());
                drag_window(ctx, &strip);
                let search = self.search_field(ui);
                if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::F)) { search.request_focus(); }
                ui.add_space(8.0);
                egui::TopBottomPanel::bottom("sidebar_footer").frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6))).show_inside(ui, |ui| {
                    ui.vertical_centered(|ui| { ui.label(egui::RichText::new(concat!("DevUtils ", env!("CARGO_PKG_VERSION"))).small().color(theme::SUBTLE)); });
                });
                let query = self.search.to_lowercase();
                let visible: Vec<usize> = (0..self.tools.len()).filter(|&i| self.tools[i].name().to_lowercase().contains(&query)).collect();
                let mut clicked = None;
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for group in Category::ALL {
                        let mut members = visible.iter().copied().filter(|&i| self.tools[i].category() == group).peekable();
                        if members.peek().is_none() { continue; }
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(group.label()).small().strong().color(theme::SUBTLE));
                        ui.add_space(3.0);
                        for i in members {
                            if self.tool_row(ui, i).clicked() { clicked = Some(i); }
                        }
                    }
                });
                if let Some(i) = clicked { self.open(ctx, Some(i)); }
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
        let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.12);
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

    /// The home screen: logo, the paste-or-drop card, recent tools and every tool as a card.
    fn home(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let area = ui.max_rect();
        let mut clicked = None;
        let mut paste = false;
        let mut palette = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let width = (ui.available_width() - 8.0).min(900.0);
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
                    // Every tool, grouped like the sidebar.
                    const GAP: f32 = 10.0;
                    let columns = ((width + GAP) / (210.0 + GAP)).floor().max(2.0) as usize;
                    let card_width = (width - GAP * (columns - 1) as f32) / columns as f32;
                    for group in Category::ALL {
                        let members: Vec<usize> = (0..self.tools.len()).filter(|&i| self.tools[i].category() == group).collect();
                        ui.add_space(18.0);
                        section_title(ui, group.label(), group.color());
                        for row in members.chunks(columns) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = GAP;
                                for &i in row {
                                    if tool_card(ui, self.tools[i].as_ref(), card_width).clicked() { clicked = Some(i); }
                                }
                            });
                            ui.add_space(GAP - ui.spacing().item_spacing.y);
                        }
                    }
                    ui.add_space(24.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("⌘K search  ·  ⌘V paste anything  ·  ⌘1 sidebar  ·  ⌘⇧F filter the sidebar").small().color(theme::SUBTLE));
                    });
                    ui.add_space(12.0);
                });
            });
        });
        if let Some(i) = clicked { self.open(&ctx, Some(i)); }
        if palette { self.palette = Some(Palette::default()); }
        if paste {
            match ui::clipboard_text() {
                Ok(text) => self.open_text(&ctx, &text),
                Err(e) => ui::toast(&ctx, icon::WARNING, e),
            }
        }
        ui::drop_hint(&ctx, area, "Drop files: one to open, two to compare");
    }

    /// Home button, sidebar toggle and the current tool's name centered, like a
    /// native title bar. The name pill opens the palette.
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let (bar_rect, bar) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_BAR), egui::Sense::click_and_drag());
        drag_window(ui.ctx(), &bar);
        let (glyph, title) = self.selected.map_or((icon::HOUSE, "DevUtils"), |i| (self.tools[i].icon(), self.tools[i].name()));
        let pill = egui::Rect::from_center_size(bar_rect.center(), egui::vec2((bar_rect.width() - 160.0).clamp(80.0, 380.0), 22.0));
        let response = ui.interact(pill, ui.id().with("title_pill"), egui::Sense::click()).on_hover_text("Search tools and commands (⌘K)");
        let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.12);
        let painter = ui.painter();
        painter.rect_filled(pill, 5.0, theme::PILL.lerp_to_gamma(theme::BUTTON, hover));
        painter.text(pill.center(), egui::Align2::CENTER_CENTER, format!("{glyph}  {title}"), egui::FontId::proportional(12.0), theme::TEXT);
        if pill.width() > 220.0 {
            painter.text(pill.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, "⌘K", egui::FontId::proportional(11.0), theme::SUBTLE.gamma_multiply(0.5 + 0.5 * hover));
        }
        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { self.palette = Some(Palette::default()); }

        let mut x = bar_rect.left();
        for (glyph, tip, action) in [(icon::SIDEBAR_SIMPLE, "Toggle sidebar (⌘1)", 0), (icon::HOUSE, "Home", 1)] {
            let button = egui::Rect::from_min_size(egui::pos2(x, bar_rect.center().y - 13.0), egui::vec2(28.0, 26.0));
            x += 30.0;
            let response = ui.interact(button, ui.id().with(("title_button", action)), egui::Sense::click()).on_hover_text(tip);
            if response.hovered() { ui.painter().rect_filled(button, 5.0, theme::BUTTON_HOVER); }
            ui.painter().text(button.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(17.0), theme::ICON);
            if response.clicked() {
                if action == 0 { self.show_sidebar = !self.show_sidebar; } else { self.open(&ui.ctx().clone(), None); }
            }
        }
    }

    /// Palette rows for `query`: best matches first. With no query: recent
    /// tools, the commands, then every tool in sidebar order.
    fn palette_entries(&self, query: &str) -> Vec<Command> {
        let commands = [Command::PasteAndDetect, Command::Home, Command::ToggleSidebar];
        let by_category = Category::ALL.into_iter().flat_map(|c| (0..self.tools.len()).filter(move |&i| self.tools[i].category() == c));
        let query = query.trim();
        if query.is_empty() {
            let recent = self.recent.iter().map(|&i| Command::Tool(i));
            let others: Vec<usize> = by_category.filter(|i| !self.recent.contains(i)).collect();
            return recent.chain(commands).chain(others.into_iter().map(Command::Tool)).collect();
        }
        let mut scored: Vec<(i32, Command)> = by_category.map(Command::Tool).chain(commands).filter_map(|command| {
            let (_, name, detail, _) = self.describe(command);
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
                (tool.icon(), tool.name().to_owned(), tool.category().label().to_owned(), tool.category().color())
            }
            Command::PasteAndDetect => (icon::MAGIC_WAND, "Paste and detect".into(), "Open the clipboard in the right tool".into(), theme::ACCENT),
            Command::Home => (icon::HOUSE, "Home".into(), "Back to the home screen".into(), theme::ACCENT),
            Command::ToggleSidebar => (icon::SIDEBAR_SIMPLE, "Toggle sidebar".into(), "⌘1".into(), theme::ACCENT),
        }
    }

    fn run(&mut self, ctx: &egui::Context, command: Command) {
        match command {
            Command::Tool(i) => self.open(ctx, Some(i)),
            Command::Home => self.open(ctx, None),
            Command::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Command::PasteAndDetect => match ui::clipboard_text() {
                Ok(text) => self.open_text(ctx, &text),
                Err(e) => ui::toast(ctx, icon::WARNING, e),
            },
        }
    }

    /// Draws the ⌘K palette when it is open, and runs the chosen command.
    fn palette(&mut self, ctx: &egui::Context) {
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

    /// In `--screenshot` mode: wait for the UI to settle, capture, save and quit.
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.screenshot else { return };
        shot.frames += 1;
        if shot.frames == 120 { ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default())); }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                if let Some(parent) = std::path::Path::new(&shot.path).parent() { let _ = std::fs::create_dir_all(parent); }
                let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Err(e) = image::save_buffer(&shot.path, &pixels, image.width() as u32, image.height() as u32, image::ColorType::Rgba8) {
                    eprintln!("Screenshot failed: {e}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint();
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

/// The title bar is transparent, so let its empty areas move and zoom the window.
fn drag_window(ctx: &egui::Context, response: &egui::Response) {
    if response.drag_started_by(egui::PointerButton::Primary) { ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag); }
    if response.double_clicked() {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_screenshot(ctx);
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Num1)) { self.show_sidebar = !self.show_sidebar; }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            self.palette = if self.palette.is_some() { None } else { Some(Palette::default()) };
        }
        if self.selected.is_none() && self.palette.is_none() { self.home_input(ctx); }
        if self.show_sidebar { self.sidebar(ctx); }
        let frame = egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin { left: 12, right: 12, top: 0, bottom: 12 });
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if !self.show_sidebar && cfg!(target_os = "macos") {
                // Without the sidebar the traffic lights sit over the central panel.
                ui.horizontal(|ui| { ui.add_space(TRAFFIC_LIGHTS); ui.vertical(|ui| self.title_bar(ui)); });
            } else {
                self.title_bar(ui);
            }
            ui.add_space(8.0);
            // A newly opened tool fades in while sliding up a little.
            let t = ((ui.input(|i| i.time) - self.opened_at) / FADE_IN).clamp(0.0, 1.0) as f32;
            let eased = 1.0 - (1.0 - t).powi(2);
            if t < 1.0 { ui.ctx().request_repaint(); }
            ui.scope(|ui| {
                ui.multiply_opacity(eased);
                ui.add_space(8.0 * (1.0 - eased));
                match self.selected {
                    Some(selected) => self.tools[selected].ui(ui),
                    None => self.home(ui),
                }
            });
        });
        self.palette(ctx);
        // A ⇄ menu asked to send a result to another tool: fill it and open it.
        if let Some(request) = tools::take_send_request(ctx) {
            if let Some(i) = self.tool_index(request.target) {
                self.tools[i].set_input(&request.text);
                self.open(ctx, Some(i));
            }
        }
        ui::show_toast(ctx);
    }
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let checking = args.screenshot.is_some();

    // Decode the icon once: it becomes the window/Dock icon and the home-screen logo.
    let icon = image::load_from_memory(ICON_PNG).expect("the bundled application icon must be a valid PNG").into_rgba8();
    let window_icon = image::imageops::resize(&icon, 256, 256, image::imageops::FilterType::Lanczos3);
    let window_icon = egui::IconData { rgba: window_icon.into_raw(), width: 256, height: 256 };

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([760.0, 480.0])
        .with_icon(window_icon)
        // Content extends under a transparent title bar, like native macOS apps.
        .with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false)
        .with_active(!checking);
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    // In screenshot mode, do not steal focus from the app the user is working in.
    #[cfg(target_os = "macos")]
    if checking { options.event_loop_builder = Some(Box::new(|builder| { use winit::platform::macos::EventLoopBuilderExtMacOS; builder.with_activate_ignoring_other_apps(false); })); }
    #[cfg(target_os = "windows")]
    if checking { options.viewport = options.viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom); }
    eframe::run_native("DevUtils", options, Box::new(move |cc| Ok(Box::new(App::new(cc, &icon, args)))))
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
