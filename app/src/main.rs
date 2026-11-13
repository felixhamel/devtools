//! DevUtils: a native collection of small developer tools, built with egui.
//!
//! `main` opens the window; `App` draws the sidebar (searchable tool list),
//! the title bar and the selected tool. Tools live in `tools/`, shared widgets
//! and colors in `ui.rs`.
use eframe::egui;

mod tools;
mod ui;

use tools::{Category, Tool};
use ui::theme;

/// Height of the transparent title bar area (the traffic-light buttons sit in it on macOS).
const TITLE_BAR: f32 = 30.0;
/// Width reserved for the traffic-light buttons when the sidebar is hidden.
const TRAFFIC_LIGHTS: f32 = 70.0;
const ICON_PNG: &[u8] = include_bytes!("../assets/app-icon.png");

struct App {
    tools: Vec<Box<dyn Tool>>,
    /// Index into `tools`; `None` shows the welcome screen.
    selected: Option<usize>,
    search: String,
    show_sidebar: bool,
    logo: egui::TextureHandle,
    screenshot: Option<Screenshot>,
}

/// `--screenshot <path>` mode: render, save a PNG of the window and exit.
/// Used to check that a build starts and draws.
struct Screenshot { path: String, frames: u32 }

impl App {
    fn new(cc: &eframe::CreationContext, icon: &image::RgbaImage, screenshot: Option<String>) -> Self {
        ui::apply_visuals(&cc.egui_ctx);
        ui::install_system_fonts(&cc.egui_ctx);
        let logo = image::imageops::resize(icon, 144, 144, image::imageops::FilterType::Lanczos3);
        let logo = egui::ColorImage::from_rgba_unmultiplied([logo.width() as usize, logo.height() as usize], logo.as_raw());
        Self {
            tools: tools::registry(),
            selected: None,
            search: String::new(),
            show_sidebar: true,
            logo: cc.egui_ctx.load_texture("logo", logo, Default::default()),
            screenshot: screenshot.map(|path| Screenshot { path, frames: 0 }),
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
                    ui.vertical_centered(|ui| { ui.label(egui::RichText::new("DevUtils").small().color(theme::SUBTLE)); });
                });
                let query = self.search.to_lowercase();
                let visible: Vec<usize> = (0..self.tools.len()).filter(|&i| self.tools[i].name().to_lowercase().contains(&query)).collect();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for group in Category::ALL {
                        let mut members = visible.iter().copied().filter(|&i| self.tools[i].category() == group).peekable();
                        if members.peek().is_none() { continue; }
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(group.label()).small().strong().color(theme::SUBTLE));
                        ui.add_space(3.0);
                        for i in members {
                            if self.tool_row(ui, i).clicked() { self.selected = Some(i); }
                        }
                    }
                });
            });
    }

    /// The rounded search box with a painted magnifying glass.
    fn search_field(&mut self, ui: &mut egui::Ui) -> egui::Response {
        egui::Frame::new()
            .fill(theme::SEARCH_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::SEARCH_STROKE))
            .corner_radius(7).inner_margin(egui::Margin::symmetric(9, 7))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon, _) = ui.allocate_exact_size(egui::vec2(15.0, 17.0), egui::Sense::hover());
                    let stroke = egui::Stroke::new(1.5_f32, theme::SEARCH_ICON);
                    let center = icon.center() - egui::vec2(1.5, 1.5);
                    ui.painter().circle_stroke(center, 4.5, stroke);
                    ui.painter().line_segment([center + egui::vec2(3.2, 3.2), center + egui::vec2(6.0, 6.0)], stroke);
                    ui.add(egui::TextEdit::singleline(&mut self.search).frame(false)
                        .hint_text("Search…  ⌘⇧F").desired_width(ui.available_width()))
                }).inner
            }).inner
    }

    /// One clickable sidebar entry: highlight, badge and tool name.
    fn tool_row(&self, ui: &mut egui::Ui, i: usize) -> egui::Response {
        let tool = &self.tools[i];
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
        let painter = ui.painter();
        if self.selected == Some(i) { painter.rect_filled(rect, 4.0, theme::ROW_SELECTED); }
        else if response.hovered() { painter.rect_filled(rect, 4.0, theme::ROW_HOVER); }
        let badge = egui::Rect::from_min_size(rect.min + egui::vec2(2.0, 3.0), egui::vec2(29.0, 20.0));
        painter.rect_filled(badge, 4.0, theme::BADGE_BG);
        painter.text(badge.center(), egui::Align2::CENTER_CENTER, tool.badge(), egui::FontId::monospace(10.0), theme::ACCENT);
        painter.text(rect.left_center() + egui::vec2(38.0, 0.0), egui::Align2::LEFT_CENTER, tool.name(), egui::FontId::proportional(13.0), egui::Color32::LIGHT_GRAY);
        response
    }

    fn welcome(&self, ui: &mut egui::Ui) {
        for tip in ["Press ⌘1 to toggle sidebar", "Press ⌘⇧F to search tools"] { ui.label(egui::RichText::new(tip).color(theme::MUTED)); }
        ui.add_space((ui.available_height() / 2.0 - 110.0).max(0.0));
        ui.vertical_centered(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(egui::vec2(72.0, 72.0)));
            ui.add_space(6.0);
            ui.label(egui::RichText::new("DevUtils").size(20.0).color(theme::TEXT_STRONG));
            ui.add_space(10.0);
            ui.label(egui::RichText::new(format!("{} tools", self.tools.len())).color(theme::MUTED));
        });
    }

    /// Sidebar toggle on the left and the current tool's name centered, like a native title bar.
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let (bar_rect, bar) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_BAR), egui::Sense::click_and_drag());
        drag_window(ui.ctx(), &bar);
        let title = self.selected.map_or("DevUtils", |i| self.tools[i].name());
        let pill = egui::Rect::from_center_size(bar_rect.center(), egui::vec2((bar_rect.width() - 120.0).clamp(80.0, 360.0), 22.0));
        ui.painter().rect_filled(pill, 5.0, theme::PILL);
        ui.painter().text(pill.center(), egui::Align2::CENTER_CENTER, title, egui::FontId::proportional(12.0), theme::TEXT);

        // Sidebar icon: a rounded rectangle with a vertical divider.
        let toggle = egui::Rect::from_min_size(egui::pos2(bar_rect.left(), bar_rect.center().y - 13.0), egui::vec2(28.0, 26.0));
        let response = ui.interact(toggle, ui.id().with("toggle_sidebar"), egui::Sense::click()).on_hover_text("Toggle sidebar (⌘1)");
        if response.hovered() { ui.painter().rect_filled(toggle, 5.0, theme::BUTTON_HOVER); }
        let icon = toggle.shrink2(egui::vec2(7.0, 8.0));
        let stroke = egui::Stroke::new(1.3_f32, theme::ICON);
        ui.painter().rect_stroke(icon, 2.0, stroke, egui::StrokeKind::Inside);
        ui.painter().line_segment([egui::pos2(icon.left() + 5.0, icon.top()), egui::pos2(icon.left() + 5.0, icon.bottom())], stroke);
        if response.clicked() { self.show_sidebar = !self.show_sidebar; }
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
            match self.selected {
                Some(selected) => self.tools[selected].ui(ui),
                None => self.welcome(ui),
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let screenshot = std::env::args().skip(1).collect::<Vec<_>>().windows(2).find(|w| w[0] == "--screenshot").map(|w| w[1].clone());
    let checking = screenshot.is_some();

    // Decode the icon once: it becomes the window/Dock icon and the welcome-screen logo.
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
    eframe::run_native("DevUtils", options, Box::new(move |cc| Ok(Box::new(App::new(cc, &icon, screenshot)))))
}
