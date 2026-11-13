use eframe::egui;

mod tools;
mod ui;

use tools::{Category, Tool};

/// Height of the transparent title bar area (the traffic-light buttons sit in it on macOS).
const TITLE_BAR: f32 = 30.0;

struct App { tools: Vec<Box<dyn Tool>>, selected: Option<usize>, search: String, show_sidebar: bool, initialized: bool, screenshot: Option<String>, frames: u32, logo: Option<egui::TextureHandle> }

impl App {
    fn new(screenshot: Option<String>) -> Self {
        Self { tools: tools::registry(), selected: None, search: String::new(), show_sidebar: false, initialized: false, screenshot, frames: 0, logo: None }
    }

    fn matches(&self, i: usize, group: Category) -> bool {
        self.tools[i].category() == group && self.tools[i].name().to_lowercase().contains(&self.search.to_lowercase())
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools").resizable(true).default_width(260.0).min_width(180.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(37, 37, 39)).inner_margin(egui::Margin { left: 12, right: 12, top: 4, bottom: 12 }))
            .show(ctx, |ui| {
                let (_, strip) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_BAR - 4.0), egui::Sense::click_and_drag());
                drag_window(ctx, &strip);
                let search = egui::Frame::new()
                    .fill(egui::Color32::from_rgb(31, 33, 32))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 80, 75)))
                    .corner_radius(7).inner_margin(egui::Margin::symmetric(9, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon, _) = ui.allocate_exact_size(egui::vec2(15.0, 17.0), egui::Sense::hover());
                            let stroke = egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(151, 164, 155));
                            let center = icon.center() - egui::vec2(1.5, 1.5);
                            ui.painter().circle_stroke(center, 4.5, stroke);
                            ui.painter().line_segment([center + egui::vec2(3.2, 3.2), center + egui::vec2(6.0, 6.0)], stroke);
                            ui.add(egui::TextEdit::singleline(&mut self.search).frame(false)
                                .hint_text("Search…  ⌘⇧F").desired_width(ui.available_width()))
                        }).inner
                    }).inner;
                if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::F)) { search.request_focus(); }
                ui.add_space(8.0);
                egui::TopBottomPanel::bottom("sidebar_footer").frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6))).show_inside(ui, |ui| {
                    ui.vertical_centered(|ui| { ui.label(egui::RichText::new("DevUtils").small().color(egui::Color32::GRAY)); });
                });
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for group in Category::ALL {
                        if !(0..self.tools.len()).any(|i| self.matches(i, group)) { continue; }
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(group.label()).small().strong().color(egui::Color32::GRAY));
                        ui.add_space(3.0);
                        for i in 0..self.tools.len() {
                            if !self.matches(i, group) { continue; }
                            let tool = &self.tools[i];
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
                            if self.selected == Some(i) || response.hovered() {
                                ui.painter().rect_filled(rect, 4.0, if self.selected == Some(i) { egui::Color32::from_rgb(65, 65, 68) } else { egui::Color32::from_rgb(48, 48, 50) });
                            }
                            let badge = egui::Rect::from_min_size(rect.min + egui::vec2(2.0, 3.0), egui::vec2(29.0, 20.0));
                            ui.painter().rect_filled(badge, 4.0, egui::Color32::from_rgb(52, 75, 65));
                            ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, tool.badge(), egui::FontId::monospace(10.0), egui::Color32::from_rgb(130, 220, 169));
                            ui.painter().text(rect.left_center() + egui::vec2(38.0, 0.0), egui::Align2::LEFT_CENTER, tool.name(), egui::FontId::proportional(13.0), egui::Color32::LIGHT_GRAY);
                            if response.clicked() { self.selected = Some(i); }
                        }
                    }
                });
            });
    }

    fn welcome(&self, ui: &mut egui::Ui) {
        for tip in ["Press ⌘1 to toggle sidebar", "Press ⌘⇧F to search tools"] { ui.label(egui::RichText::new(tip).color(ui::MUTED)); }
        ui.add_space((ui.available_height() / 2.0 - 110.0).max(0.0));
        ui.vertical_centered(|ui| {
            if let Some(logo) = &self.logo { ui.add(egui::Image::new(logo).fit_to_exact_size(egui::vec2(72.0, 72.0))); }
            ui.add_space(6.0);
            ui.label(egui::RichText::new("DevUtils").size(20.0).color(egui::Color32::from_rgb(230, 230, 230)));
            ui.add_space(10.0);
            ui.label(egui::RichText::new(format!("{} tools", self.tools.len())).color(ui::MUTED));
        });
    }

    /// Sidebar toggle on the left and the current tool's name centered, like a native title bar.
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let (bar_rect, bar) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_BAR), egui::Sense::click_and_drag());
        drag_window(ui.ctx(), &bar);
        let title = self.selected.map_or("DevUtils", |i| self.tools[i].name());
        let pill = egui::Rect::from_center_size(bar_rect.center(), egui::vec2((bar_rect.width() - 120.0).clamp(80.0, 360.0), 22.0));
        ui.painter().rect_filled(pill, 5.0, egui::Color32::from_rgb(58, 58, 58));
        ui.painter().text(pill.center(), egui::Align2::CENTER_CENTER, title, egui::FontId::proportional(12.0), egui::Color32::from_rgb(220, 220, 220));
        let toggle = egui::Rect::from_min_size(egui::pos2(bar_rect.left(), bar_rect.center().y - 13.0), egui::vec2(28.0, 26.0));
        let response = ui.interact(toggle, ui.id().with("toggle_sidebar"), egui::Sense::click()).on_hover_text("Toggle sidebar (⌘1)");
        if response.hovered() { ui.painter().rect_filled(toggle, 5.0, egui::Color32::from_rgb(62, 62, 64)); }
        let icon = toggle.shrink2(egui::vec2(7.0, 8.0));
        let stroke = egui::Stroke::new(1.3_f32, egui::Color32::from_rgb(180, 180, 182));
        ui.painter().rect_stroke(icon, 2.0, stroke, egui::StrokeKind::Inside);
        ui.painter().line_segment([egui::pos2(icon.left() + 5.0, icon.top()), egui::pos2(icon.left() + 5.0, icon.bottom())], stroke);
        if response.clicked() { self.show_sidebar = !self.show_sidebar; }
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
        if self.screenshot.is_some() {
            self.frames += 1;
            if self.frames == 120 { ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default())); }
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Some(path) = &self.screenshot {
                        if let Some(parent) = std::path::Path::new(path).parent() { let _ = std::fs::create_dir_all(parent); }
                        if let Err(e) = image::save_buffer(path, &pixels, image.width() as u32, image.height() as u32, image::ColorType::Rgba8) { eprintln!("Screenshot failed: {e}"); }
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            ctx.request_repaint();
        }
        if !self.initialized {
            self.show_sidebar = true;
            self.initialized = true;
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(46, 44, 44);
            visuals.window_fill = egui::Color32::from_rgb(46, 44, 44);
            ctx.set_visuals(visuals);
            ui::install_system_fonts(ctx);
            let logo = image::load_from_memory(include_bytes!("../assets/app-icon.png")).expect("bundled icon").resize(144, 144, image::imageops::FilterType::Lanczos3).into_rgba8();
            self.logo = Some(ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([logo.width() as usize, logo.height() as usize], logo.as_raw()), Default::default()));
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Num1)) { self.show_sidebar = !self.show_sidebar; }
        if self.show_sidebar { self.sidebar(ctx); }
        egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin { left: 12, right: 12, top: 0, bottom: 12 })).show(ctx, |ui| {
            // Leave room for the traffic lights when the sidebar is hidden.
            if !self.show_sidebar && cfg!(target_os = "macos") {
                ui.horizontal(|ui| { ui.add_space(70.0); ui.vertical(|ui| self.title_bar(ui)); });
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
    let icon = image::load_from_memory(include_bytes!("../assets/app-icon.png"))
        .expect("the bundled application icon must be a valid PNG")
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let icon = egui::IconData { rgba: icon.into_raw(), width: 256, height: 256 };
    let mut options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]).with_min_inner_size([760.0, 480.0]).with_icon(icon)
        .with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false).with_active(!checking), ..Default::default() };
    #[cfg(target_os = "macos")]
    if checking { options.event_loop_builder = Some(Box::new(|builder| { use winit::platform::macos::EventLoopBuilderExtMacOS; builder.with_activate_ignoring_other_apps(false); })); }
    #[cfg(target_os = "windows")]
    if checking { options.viewport = options.viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom); }
    eframe::run_native("DevUtils", options, Box::new(move |_| Ok(Box::new(App::new(screenshot)))))
}
