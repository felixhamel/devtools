//! DevUtils: a native collection of small developer tools, built with egui.
//!
//! `main` opens the window; `App` draws the sidebar (searchable tool list),
//! the title bar and the selected tool. Tools live in `tools/`, shared widgets
//! and colors in `ui.rs`.
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

/// Command-line options, mostly for checking the interface:
/// `--tool <name>` opens a tool (name matched case-insensitively),
/// `--sample` loads its example, `--screenshot <path>` captures the window and quits.
#[derive(Default)]
struct Args { screenshot: Option<String>, tool: Option<String>, sample: bool }

impl Args {
    fn parse() -> Self {
        let mut args = Args::default();
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--screenshot" => args.screenshot = it.next(),
                "--tool" => args.tool = it.next(),
                "--sample" => args.sample = true,
                _ => {}
            }
        }
        args
    }
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
            search: String::new(),
            show_sidebar: true,
            logo: cc.egui_ctx.load_texture("logo", logo, Default::default()),
            screenshot: args.screenshot.map(|path| Screenshot { path, frames: 0 }),
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

    /// One clickable sidebar entry: icon and tool name, mint-tinted when selected.
    fn tool_row(&self, ui: &mut egui::Ui, i: usize) -> egui::Response {
        let tool = &self.tools[i];
        let selected = self.selected == Some(i);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
        let painter = ui.painter();
        if selected { painter.rect_filled(rect, 5.0, theme::ROW_SELECTED); }
        else if response.hovered() { painter.rect_filled(rect, 5.0, theme::ROW_HOVER); }
        let (icon_color, text_color) = if selected { (theme::ACCENT, theme::TEXT_STRONG) } else { (theme::ICON, egui::Color32::LIGHT_GRAY) };
        painter.text(rect.left_center() + egui::vec2(16.0, 0.0), egui::Align2::CENTER_CENTER, tool.icon(), egui::FontId::proportional(16.0), icon_color);
        painter.text(rect.left_center() + egui::vec2(32.0, 0.0), egui::Align2::LEFT_CENTER, tool.name(), egui::FontId::proportional(13.0), text_color);
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    fn welcome(&self, ui: &mut egui::Ui) {
        for tip in ["Press ⌘1 to toggle sidebar", "Press ⌘⇧F to search tools"] {
            ui.label(egui::RichText::new(format!("{}  {tip}", icon::KEYBOARD)).color(theme::MUTED));
        }
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

        let toggle = egui::Rect::from_min_size(egui::pos2(bar_rect.left(), bar_rect.center().y - 13.0), egui::vec2(28.0, 26.0));
        let response = ui.interact(toggle, ui.id().with("toggle_sidebar"), egui::Sense::click()).on_hover_text("Toggle sidebar (⌘1)");
        if response.hovered() { ui.painter().rect_filled(toggle, 5.0, theme::BUTTON_HOVER); }
        ui.painter().text(toggle.center(), egui::Align2::CENTER_CENTER, icon::SIDEBAR_SIMPLE, egui::FontId::proportional(17.0), theme::ICON);
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
        // A ⇄ menu asked to send a result to another tool: fill it and open it.
        if let Some(request) = tools::take_send_request(ctx) {
            if let Some(i) = self.tools.iter().position(|t| t.name() == request.target) {
                self.tools[i].set_input(&request.text);
                self.selected = Some(i);
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let checking = args.screenshot.is_some();

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
    eframe::run_native("DevUtils", options, Box::new(move |cc| Ok(Box::new(App::new(cc, &icon, args)))))
}
