//! `DevUtils`: a native collection of small developer tools, built with egui.
//!
//! `main` opens the window; [`App`] holds the app state and draws each frame.
//! Its drawing is split by area: [`chrome`] (sidebar and title bar), [`home`]
//! (home screen, paste and drop), [`palette`] (⌘K palette); [`state`] is what
//! is saved between launches. Tools live in [`tools`], shared widgets and
//! colors in [`ui`].
use eframe::egui;

mod chrome;
mod home;
mod palette;
mod state;
mod tools;
mod ui;

use palette::Palette;
use state::Saved;
use tools::Tool;
use ui::icon;

/// Height of the transparent title bar area (the traffic-light buttons sit in it on macOS).
const TITLE_BAR: f32 = 30.0;
/// Width reserved for the traffic-light buttons when the sidebar is hidden.
const TRAFFIC_LIGHTS: f32 = 70.0;
/// The application icon: window and Dock icon, and the home-screen logo.
const ICON_PNG: &[u8] = include_bytes!("../assets/app-icon.png");
/// Seconds a newly opened tool takes to fade in.
const FADE_IN: f64 = 0.18;
/// How many tools the "Recent" list keeps.
const MAX_RECENT: usize = 6;
/// Frames drawn before the `--screenshot` capture, so fonts, textures and
/// animations have settled.
const SCREENSHOT_AFTER_FRAMES: u32 = 120;

/// The application: every tool plus the navigation state around them.
struct App {
    /// Every tool, in registry order; the indices below point into it.
    tools: Vec<Box<dyn Tool>>,
    /// Index into `tools`; `None` shows the home screen.
    selected: Option<usize>,
    /// Recently opened tools, most recent first.
    recent: Vec<usize>,
    /// Starred tools, in the order they were starred.
    favorites: Vec<usize>,
    /// When the current tool was opened (`ctx` time), for the fade-in.
    opened_at: f64,
    /// Filter typed in the sidebar's search box.
    search: String,
    show_sidebar: bool,
    /// `Some` while the ⌘K palette is open.
    palette: Option<Palette>,
    /// The app icon, drawn on the home screen.
    logo: egui::TextureHandle,
    /// `Some` in `--screenshot` mode.
    screenshot: Option<Screenshot>,
    /// `--screenshot` mode: the saved state is neither read nor written.
    checking: bool,
}

/// `--screenshot <path>` mode: render, save a PNG of the window and exit.
/// Used to check that a build starts and draws.
struct Screenshot {
    /// Where the PNG goes; missing parent folders are created.
    path: String,
    /// Frames drawn so far (see [`SCREENSHOT_AFTER_FRAMES`]).
    frames: u32,
}

/// Command-line options, mostly for checking the interface:
/// `--tool <name>` opens a tool (name matched case-insensitively),
/// `--sample` loads its example, `--palette` opens the ⌘K palette,
/// `--screenshot <path>` captures the window and quits.
#[derive(Default)]
struct Args { screenshot: Option<String>, tool: Option<String>, sample: bool, palette: bool }

impl Args {
    /// Reads the process arguments; unknown ones are ignored.
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

impl App {
    /// Sets up the style and fonts, builds the tools and restores the saved
    /// state (unless `--screenshot`); `--tool` and `--sample` apply on top.
    fn new(cc: &eframe::CreationContext, icon: &image::RgbaImage, args: Args) -> Self {
        ui::apply_visuals(&cc.egui_ctx);
        ui::install_fonts(&cc.egui_ctx);
        // The logo is drawn at 72 points: 144 pixels keep it sharp on Retina screens.
        let logo = image::imageops::resize(icon, 144, 144, image::imageops::FilterType::Lanczos3);
        let logo = egui::ColorImage::from_rgba_unmultiplied([logo.width() as usize, logo.height() as usize], logo.as_raw());
        let mut tools = tools::registry();
        tools::publish_directory(&cc.egui_ctx, &tools);
        let checking = args.screenshot.is_some();
        let saved = if checking { Saved::default() } else { Saved::load(cc.storage) };
        let (mut recent, favorites) = (state::indices(&tools, &saved.recent), state::indices(&tools, &saved.favorites));
        // `--tool` wins over the tool that was open last time.
        let selected = match args.tool {
            Some(wanted) => tools.iter().position(|t| t.name().eq_ignore_ascii_case(&wanted)),
            None => saved.last_tool.and_then(|name| tools.iter().position(|t| t.name() == name)),
        };
        if let (Some(i), true) = (selected, args.sample) { tools[i].load_sample(); }
        if let Some(i) = selected {
            recent.retain(|&r| r != i);
            recent.insert(0, i);
            recent.truncate(MAX_RECENT);
        }
        Self {
            tools,
            selected,
            recent,
            favorites,
            opened_at: f64::NEG_INFINITY,
            search: String::new(),
            show_sidebar: !saved.sidebar_hidden,
            palette: args.palette.then(Palette::default),
            logo: cc.egui_ctx.load_texture("logo", logo, egui::TextureOptions::default()),
            screenshot: args.screenshot.map(|path| Screenshot { path, frames: 0 }),
            checking,
        }
    }

    /// Stars or unstars tool `i`.
    fn toggle_favorite(&mut self, ctx: &egui::Context, i: usize) {
        let name = self.tools[i].name();
        if let Some(at) = self.favorites.iter().position(|&f| f == i) {
            self.favorites.remove(at);
            ui::toast(ctx, icon::STAR, format!("Removed {name} from favorites"));
        } else {
            self.favorites.push(i);
            ui::toast(ctx, icon::STAR, format!("Added {name} to favorites"));
        }
    }

    /// The right-click menu of a tool in the sidebar or on the home screen;
    /// returns the tool to star or unstar.
    fn favorite_menu(&self, response: &egui::Response, i: usize) -> Option<usize> {
        let mut toggle = None;
        response.context_menu(|ui| {
            let label = if self.favorites.contains(&i) { "Remove from favorites" } else { "Add to favorites" };
            if ui.button(format!("{}  {label}", icon::STAR)).clicked() {
                toggle = Some(i);
                ui.close_menu();
            }
        });
        toggle
    }

    /// Index of the tool called `name` (exact match), if any.
    fn tool_index(&self, name: &str) -> Option<usize> { self.tools.iter().position(|t| t.name() == name) }

    /// Shows tool `index` (or the home screen for `None`) and remembers it as recent.
    fn open(&mut self, ctx: &egui::Context, index: Option<usize>) {
        if self.selected != index { self.opened_at = ctx.input(|i| i.time); }
        self.selected = index;
        if let Some(i) = index {
            self.recent.retain(|&r| r != i);
            self.recent.insert(0, i);
            self.recent.truncate(MAX_RECENT);
        }
    }

    /// In `--screenshot` mode: wait for the UI to settle, capture, save and quit.
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.screenshot else { return };
        shot.frames += 1;
        if shot.frames == SCREENSHOT_AFTER_FRAMES { ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default())); }
        // The capture arrives as an event a frame or more later.
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                if let Some(parent) = std::path::Path::new(&shot.path).parent() { let _ = std::fs::create_dir_all(parent); }
                let pixels: Vec<u8> = image.pixels.iter().flat_map(egui::Color32::to_array).collect();
                if let Err(e) = image::save_buffer(&shot.path, &pixels, image.width() as u32, image.height() as u32, image::ColorType::Rgba8) {
                    eprintln!("Screenshot failed: {e}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    /// Draws one frame: global shortcuts, then the sidebar, the title bar and
    /// the selected tool or home screen, then the palette and the toast.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_screenshot(ctx);
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Num1)) { self.show_sidebar = !self.show_sidebar; }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            self.palette = if self.palette.is_some() { None } else { Some(Palette::default()) };
        }
        // ⌘D is only consumed while a tool is open (`filter` skips the closure otherwise).
        if let Some(i) = self.selected.filter(|_| ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D))) {
            self.toggle_favorite(ctx, i);
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

    /// Called by eframe now and then and when quitting: see [`Saved`].
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.checking { return; }
        Saved {
            last_tool: self.selected.map(|i| self.tools[i].name().to_owned()),
            recent: state::names(&self.tools, &self.recent),
            favorites: state::names(&self.tools, &self.favorites),
            sidebar_hidden: !self.show_sidebar,
        }.store(storage);
    }

    /// Panel widths and the like are kept too, except in `--screenshot` mode.
    fn persist_egui_memory(&self) -> bool { !self.checking }
}

/// Parses the arguments, decodes the icon and runs the app until the window closes.
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
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    if checking {
        // Screenshots start from a clean state and must not change the user's:
        // point eframe at a file that is never written (nothing is saved).
        options.persist_window = false;
        options.persistence_path = Some(std::env::temp_dir().join("devutils-screenshot-state.ron"));
    }
    // In screenshot mode, do not steal focus from the app the user is working in.
    #[cfg(target_os = "macos")]
    if checking { options.event_loop_builder = Some(Box::new(|builder| { use winit::platform::macos::EventLoopBuilderExtMacOS; builder.with_activate_ignoring_other_apps(false); })); }
    #[cfg(target_os = "windows")]
    if checking { options.viewport = options.viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom); }
    eframe::run_native("DevUtils", options, Box::new(move |cc| Ok(Box::new(App::new(cc, &icon, args)))))
}
