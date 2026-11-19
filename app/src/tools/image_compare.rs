//! Image Compare: two images overlaid with a draggable divider (swipe), side
//! by side, blended (onion skin) or as a map of the pixels that differ.
//! Pinch or ⌘-scroll zooms, scrolling pans, double-click fits the view again.
use eframe::egui;
use image::RgbaImage;
use super::{Category, Tool};
use crate::ui::{self, icon, theme};

const EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];
/// Colors of the A and B labels.
const SIDE_COLORS: [egui::Color32; 2] = [theme::BLUE, theme::AMBER];
/// From this magnification on, pixels are drawn as sharp squares (to inspect
/// them) instead of smoothed.
const SHARP_SCALE: f32 = 4.0;
const STATUS_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Mode { #[default] Swipe, SideBySide, Onion, Difference }

/// A loaded image: its pixels (for the difference and the color readout) and its texture.
struct Picture { name: String, info: String, pixels: RgbaImage, texture: egui::TextureHandle }

/// The difference map and how many pixels differ.
struct Difference { map: RgbaImage, texture: egui::TextureHandle, changed: usize, total: usize }

pub struct ImageCompare {
    pictures: [Option<Picture>; 2],
    mode: Mode,
    /// Divider position in the swipe view: A is shown left of it, B right of it.
    split: f32,
    /// Opacity of B over A in the onion-skin view.
    opacity: f32,
    /// Channel differences up to this value are ignored (e.g. JPEG noise).
    tolerance: u8,
    difference: Option<Difference>,
    /// 1 fits the images in the view.
    zoom: f32,
    pan: egui::Vec2,
    checker: Option<egui::TextureHandle>,
    /// Whether the textures currently use sharp (nearest) magnification.
    sharp: bool,
    error: String,
    /// The example was asked for outside `ui` (e.g. `--sample`), where textures cannot be made yet.
    pending_sample: bool,
}

impl Default for ImageCompare {
    fn default() -> Self {
        Self { pictures: [None, None], mode: Mode::default(), split: 0.5, opacity: 0.5, tolerance: 0, difference: None,
            zoom: 1.0, pan: egui::Vec2::ZERO, checker: None, sharp: false, error: String::new(), pending_sample: false }
    }
}

/// Decodes an image file into pixels and a one-line summary (format, size, bytes).
fn decode(name: &str, bytes: &[u8]) -> Result<(String, RgbaImage), String> {
    let format = image::guess_format(bytes).map_err(|_| format!("{name}: unsupported or unrecognized image format"))?;
    let pixels = image::load_from_memory_with_format(bytes, format).map_err(|e| format!("{name}: {e}"))?.into_rgba8();
    let format = format!("{format:?}").to_uppercase();
    Ok((format!("{format} · {}×{} · {}", pixels.width(), pixels.height(), ui::human_size(bytes.len())), pixels))
}

/// Uploads `pixels` as a texture, shrunk first if the GPU cannot hold it.
/// `sharp` draws magnified pixels as squares instead of smoothing them.
fn upload(ctx: &egui::Context, name: &str, pixels: &RgbaImage, sharp: bool) -> egui::TextureHandle {
    let max = ctx.input(|i| i.max_texture_side) as u32;
    let shrunk;
    let pixels = if pixels.width() > max || pixels.height() > max {
        let scale = max as f32 / pixels.width().max(pixels.height()) as f32;
        let (w, h) = (((pixels.width() as f32 * scale) as u32).max(1), ((pixels.height() as f32 * scale) as u32).max(1));
        shrunk = image::imageops::resize(pixels, w, h, image::imageops::FilterType::Triangle);
        &shrunk
    } else {
        pixels
    };
    let image = egui::ColorImage::from_rgba_unmultiplied([pixels.width() as usize, pixels.height() as usize], pixels.as_raw());
    let magnification = if sharp { egui::TextureFilter::Nearest } else { egui::TextureFilter::Linear };
    ctx.load_texture(name, image, egui::TextureOptions { magnification, ..egui::TextureOptions::LINEAR })
}

/// A map of the pixels that differ by more than `tolerance` on any channel
/// (hot pink, brighter for bigger differences) over a dimmed gray copy of A.
/// Where only one image has pixels, they count as different.
/// Returns the map and the number of differing pixels.
fn difference(a: &RgbaImage, b: &RgbaImage, tolerance: u8) -> (RgbaImage, usize) {
    let (w, h) = (a.width().max(b.width()), a.height().max(b.height()));
    let mut changed = 0;
    let map = RgbaImage::from_fn(w, h, |x, y| match (a.get_pixel_checked(x, y), b.get_pixel_checked(x, y)) {
        (Some(pa), Some(pb)) => {
            let delta = pa.0.iter().zip(pb.0).map(|(p, q)| p.abs_diff(q)).max().unwrap_or(0);
            if delta > tolerance {
                changed += 1;
                let t = 0.5 + 0.5 * delta as f32 / 255.0;
                let [r, g, b, _] = theme::HOT.to_array();
                image::Rgba([(r as f32 * t) as u8, (g as f32 * t) as u8, (b as f32 * t) as u8, 255])
            } else {
                let [r, g, b, alpha] = pa.0;
                let gray = (0.3 * r as f32 + 0.59 * g as f32 + 0.11 * b as f32) * alpha as f32 / 255.0 * 0.35;
                image::Rgba([gray as u8, gray as u8, gray as u8, 255])
            }
        }
        _ => {
            changed += 1;
            image::Rgba([110, 28, 64, 255])
        }
    });
    (map, changed)
}

/// The example pair: a small landscape, and the same at sunset with a cloud and a tree.
fn landscape(sunset: bool) -> RgbaImage {
    let inside = |x: f32, y: f32, cx: f32, cy: f32, r: f32| (x - cx).powi(2) + (y - cy).powi(2) < r * r;
    RgbaImage::from_fn(480, 300, |x, y| {
        let (x, y) = (x as f32, y as f32);
        let t = y / 300.0;
        let mut c = [0, 1, 2].map(|i| ([38.0, 64.0, 128.0][i] * (1.0 - t) + [250.0, 175.0, 125.0][i] * t) as u8);
        let (sun, color) = if sunset { ((300.0, 150.0), [255, 176, 82]) } else { ((350.0, 95.0), [255, 214, 102]) };
        if inside(x, y, sun.0, sun.1, 34.0) { c = color; }
        let cloud = inside(x, y, 118.0, 78.0, 18.0) || inside(x, y, 142.0, 66.0, 24.0) || inside(x, y, 168.0, 76.0, 19.0)
            || ((100.0..186.0).contains(&x) && (74.0..94.0).contains(&y));
        if sunset && cloud { c = [246, 238, 240]; }
        if y > 120.0 + (x - 150.0).abs() * 0.95 { c = if y < 148.0 { [236, 240, 250] } else { [72, 82, 125] }; }
        if y > 150.0 + (x - 360.0).abs() * 0.8 { c = [44, 54, 84]; }
        if y > 255.0 + (x / 40.0).sin() * 4.0 { c = [42, 74, 58]; }
        // A tree only in the second image.
        if sunset && ((84.0..90.0).contains(&x) && (236.0..262.0).contains(&y) || y > 196.0 + (x - 87.0).abs() * 2.2 && y < 240.0) {
            c = if y >= 236.0 { [96, 66, 44] } else { [36, 110, 70] };
        }
        image::Rgba([c[0], c[1], c[2], 255])
    })
}

/// A checkerboard tile, repeated under transparent images.
fn checker(ctx: &egui::Context) -> egui::TextureHandle {
    let (light, dark) = (egui::Color32::from_gray(58), egui::Color32::from_gray(44));
    let tile = egui::ColorImage { size: [2, 2], pixels: vec![light, dark, dark, light] };
    let options = egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::NEAREST };
    ctx.load_texture("compare-checker", tile, options)
}

/// Where the images are drawn in `pane`: a canvas the size of the larger
/// image, fitted, then zoomed and panned. Returns the canvas and its scale.
fn canvas(pane: egui::Rect, size: egui::Vec2, zoom: f32, pan: egui::Vec2) -> (egui::Rect, f32) {
    let scale = (pane.width() / size.x).min(pane.height() / size.y) * zoom;
    (egui::Rect::from_center_size(pane.center() + pan, size * scale), scale)
}

/// A small dark label with a colored letter, e.g. "A  before.png".
fn label(painter: &egui::Painter, anchor: egui::Pos2, align: egui::Align2, side: usize, name: &str) {
    let mut job = egui::text::LayoutJob::default();
    let font = egui::FontId::proportional(12.0);
    job.append(["A", "B"][side], 0.0, egui::TextFormat::simple(egui::FontId::proportional(12.5), SIDE_COLORS[side]));
    job.append(name, 8.0, egui::TextFormat::simple(font, theme::TEXT_STRONG));
    let galley = painter.layout_job(job);
    let rect = align.anchor_size(anchor, galley.size() + egui::vec2(16.0, 8.0));
    painter.rect_filled(rect, 6.0, egui::Color32::from_black_alpha(170));
    painter.galley(rect.min + egui::vec2(8.0, 4.0), galley, theme::TEXT);
}

impl ImageCompare {
    fn set(&mut self, ctx: &egui::Context, side: usize, name: String, info: String, pixels: RgbaImage) {
        let texture = upload(ctx, &format!("compare-{side}"), &pixels, self.sharp);
        self.pictures[side] = Some(Picture { name, info, pixels, texture });
        self.error.clear();
    }

    /// Loads files into the slots from `side` on (two files fill both).
    fn load_files(&mut self, ctx: &egui::Context, side: usize, files: Vec<Result<(String, Vec<u8>), String>>) {
        for (side, file) in (side..2).zip(files) {
            match file.and_then(|(name, bytes)| decode(&name, &bytes).map(|(info, pixels)| (name, info, pixels))) {
                Ok((name, info, pixels)) => self.set(ctx, side, name, info, pixels),
                Err(e) => self.error = e,
            }
        }
        self.refresh(ctx);
    }

    fn paste(&mut self, ctx: &egui::Context, side: usize) {
        let image = arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_image());
        match image.ok().and_then(|img| RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())) {
            Some(pixels) => {
                let info = format!("Clipboard · {}×{}", pixels.width(), pixels.height());
                self.set(ctx, side, "Pasted image".into(), info, pixels);
                self.refresh(ctx);
            }
            None => self.error = "The clipboard holds no image. Copy one (e.g. a screenshot with ⌃⇧⌘4) and paste again.".into(),
        }
    }

    /// Recomputes the difference after an image or the tolerance changed.
    fn refresh(&mut self, ctx: &egui::Context) {
        self.difference = match &self.pictures {
            [Some(a), Some(b)] => {
                let (map, changed) = difference(&a.pixels, &b.pixels, self.tolerance);
                let total = (map.width() * map.height()) as usize;
                Some(Difference { texture: upload(ctx, "compare-difference", &map, self.sharp), map, changed, total })
            }
            _ => None,
        };
    }

    /// Uploads every texture again after the magnification filter changed.
    fn reupload(&mut self, ctx: &egui::Context) {
        for (side, picture) in self.pictures.iter_mut().enumerate() {
            if let Some(p) = picture { p.texture = upload(ctx, &format!("compare-{side}"), &p.pixels, self.sharp); }
        }
        if let Some(d) = &mut self.difference { d.texture = upload(ctx, "compare-difference", &d.map, self.sharp); }
    }

    /// Files dropped on the window: two fill both slots, one goes to the first empty slot (or B).
    fn take_dropped(&mut self, ctx: &egui::Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        if files.is_empty() { return; }
        let side = if files.len() >= 2 || self.pictures[0].is_none() { 0 } else { 1 };
        self.load_files(ctx, side, files.iter().map(ui::read_dropped).collect());
    }

    fn images_toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui::toolbar(ui, "Images:", |ui| {
            for side in 0..2 {
                let letter = ["A", "B"][side];
                ui.label(egui::RichText::new(letter).strong().color(SIDE_COLORS[side]));
                if ui::icon_only(ui, icon::FOLDER_OPEN, &format!("Open image {letter} (pick two to fill both)")) {
                    let paths = ui::pick_files("Open images to compare", &EXTENSIONS);
                    if !paths.is_empty() { self.load_files(&ctx, side, paths.iter().map(|p| ui::read_path(p)).collect()); }
                }
                if ui::icon_only(ui, icon::CLIPBOARD, &format!("Paste image {letter} from the clipboard")) { self.paste(&ctx, side); }
                match &self.pictures[side] {
                    Some(p) => { ui.add(egui::Label::new(egui::RichText::new(&p.name).color(theme::TEXT)).truncate()).on_hover_text(&p.info); }
                    None => { ui.label(egui::RichText::new("empty").color(theme::SUBTLE)); }
                }
                if side == 0 && ui::icon_only(ui, icon::ARROWS_LEFT_RIGHT, "Swap A and B") {
                    self.pictures.swap(0, 1);
                    self.refresh(&ctx);
                }
            }
            ui.separator();
            if ui::sample_button(ui) { self.load_sample_with(&ctx); }
            if ui::clear_button(ui) { *self = Self { mode: self.mode, checker: self.checker.take(), sharp: self.sharp, ..Self::default() }; }
        });
    }

    fn view_toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui::toolbar(ui, "View:", |ui| {
            let names = [
                (Mode::Swipe, format!("{}  Swipe", icon::SQUARE_SPLIT_HORIZONTAL)),
                (Mode::SideBySide, format!("{}  Side by side", icon::COLUMNS)),
                (Mode::Onion, format!("{}  Onion skin", icon::STACK)),
                (Mode::Difference, format!("{}  Difference", icon::CIRCLE_HALF)),
            ];
            let options: Vec<(Mode, &str)> = names.iter().map(|(mode, name)| (*mode, name.as_str())).collect();
            ui::segmented(ui, &mut self.mode, &options);
            match self.mode {
                Mode::Onion => { ui.add(egui::Slider::new(&mut self.opacity, 0.0..=1.0).text("B opacity").show_value(false)); }
                Mode::Difference => {
                    let slider = ui.add(egui::Slider::new(&mut self.tolerance, 0..=64).text("tolerance"))
                        .on_hover_text("Ignore channel differences up to this value, e.g. JPEG noise");
                    // Recompute once the slider is released (or the value typed), not on every drag step.
                    if slider.drag_stopped() || (slider.changed() && !slider.dragged()) { self.refresh(&ctx); }
                }
                _ => {}
            }
            ui.separator();
            if ui.add(ui::icon_button(icon::CORNERS_OUT, "Fit", false)).on_hover_text("Fit the images in the view (double-click)").clicked() {
                self.zoom = 1.0;
                self.pan = egui::Vec2::ZERO;
            }
            ui.label(egui::RichText::new(format!("{:.0} %", self.zoom * 100.0)).color(theme::MUTED)).on_hover_text("Pinch or ⌘-scroll to zoom, scroll to pan");
            ui.separator();
            if let (Some(d), [Some(a), Some(b)]) = (&self.difference, &self.pictures) {
                if a.pixels.dimensions() != b.pixels.dimensions() {
                    ui::badge(ui, icon::WARNING, "Sizes differ", theme::WARNING);
                }
                if d.changed == 0 {
                    ui::badge(ui, icon::CHECK_CIRCLE, "Identical", theme::ACCENT);
                } else {
                    let percent = d.changed as f64 * 100.0 / d.total as f64;
                    let text = egui::RichText::new(format!("{percent:.2} % of pixels differ")).color(theme::HOT);
                    ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Extend)).on_hover_text(format!("{} of {} pixels", d.changed, d.total));
                }
            }
        });
    }

    /// The comparison area; returns the color readout under the pointer, if any.
    fn view(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let size = ui.available_size() - egui::vec2(0.0, STATUS_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect(rect, 6.0, theme::EDITOR_BG, egui::Stroke::new(1.0_f32, theme::EDITOR_STROKE), egui::StrokeKind::Inside);
        let view = rect.shrink(10.0);
        let loaded: Vec<&Picture> = self.pictures.iter().flatten().collect();
        if loaded.is_empty() {
            let center = view.center();
            painter.text(center - egui::vec2(0.0, 30.0), egui::Align2::CENTER_CENTER, icon::IMAGES, egui::FontId::proportional(44.0), theme::SUBTLE);
            painter.text(center + egui::vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, "Drop two images here", egui::FontId::proportional(16.0), theme::TEXT);
            painter.text(center + egui::vec2(0.0, 38.0), egui::Align2::CENTER_CENTER, "or open / paste each one above · PNG, JPEG, WebP, BMP", egui::FontId::proportional(12.5), theme::SUBTLE);
            return None;
        }
        let size = loaded.iter().fold(egui::Vec2::ZERO, |s, p| s.max(egui::vec2(p.pixels.width() as f32, p.pixels.height() as f32)));
        let both = loaded.len() == 2;
        let panes = if self.mode == Mode::SideBySide && both {
            let half = (view.width() - 10.0) / 2.0;
            vec![egui::Rect::from_min_size(view.min, egui::vec2(half, view.height())),
                 egui::Rect::from_min_size(view.min + egui::vec2(half + 10.0, 0.0), egui::vec2(half, view.height()))]
        } else {
            vec![view]
        };

        // Zoom around the pointer, pan with scrolling, double-click to fit.
        let pointer = response.hover_pos();
        if let Some(p) = pointer {
            let pane = panes.iter().find(|pane| pane.contains(p)).copied().unwrap_or(view);
            let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
            if zoom != 1.0 {
                let new_zoom = (self.zoom * zoom).clamp(0.25, 64.0);
                let k = new_zoom / self.zoom;
                self.pan = (p - pane.center()) - (p - pane.center() - self.pan) * k;
                self.zoom = new_zoom;
            } else {
                self.pan += scroll;
            }
        }
        if response.double_clicked() {
            self.zoom = 1.0;
            self.pan = egui::Vec2::ZERO;
        }
        let (_, scale) = canvas(panes[0], size, self.zoom, egui::Vec2::ZERO);
        let limit = ((size * scale - panes[0].size()) / 2.0).max(egui::Vec2::ZERO);
        self.pan = self.pan.clamp(-limit, limit);
        // Smooth when fitted, sharp squares when zoomed in on the pixels.
        if (scale * ui.ctx().pixels_per_point() >= SHARP_SCALE) != self.sharp {
            self.sharp = !self.sharp;
            self.reupload(ui.ctx());
        }

        let checker = self.checker.get_or_insert_with(|| checker(ui.ctx())).id();
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let mut readout = None;
        for (index, pane) in panes.iter().enumerate() {
            let (canvas, scale) = canvas(*pane, size, self.zoom, self.pan);
            let painter = painter.with_clip_rect(*pane);
            let checker_uv = egui::Rect::from_min_size(egui::Pos2::ZERO, canvas.size() / 16.0);
            painter.image(checker, canvas, checker_uv, egui::Color32::WHITE);
            let place = |p: &Picture| egui::Rect::from_min_size(canvas.min, egui::vec2(p.pixels.width() as f32, p.pixels.height() as f32) * scale);
            match (&self.pictures, self.mode) {
                ([Some(a), Some(b)], Mode::Swipe) => {
                    let x = (canvas.left() + self.split * canvas.width()).clamp(pane.left(), pane.right());
                    painter.with_clip_rect(pane.with_max_x(x)).image(a.texture.id(), place(a), uv, egui::Color32::WHITE);
                    painter.with_clip_rect(pane.with_min_x(x)).image(b.texture.id(), place(b), uv, egui::Color32::WHITE);
                    let (top, bottom) = (canvas.top().max(pane.top()), canvas.bottom().min(pane.bottom()));
                    painter.line_segment([egui::pos2(x, top), egui::pos2(x, bottom)], egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                    let handle = egui::pos2(x, pane.center().y.clamp(top + 16.0, (bottom - 16.0).max(top + 16.0)));
                    painter.circle(handle, 15.0, egui::Color32::WHITE, egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(60)));
                    painter.text(handle, egui::Align2::CENTER_CENTER, icon::ARROWS_LEFT_RIGHT, egui::FontId::proportional(16.0), theme::ON_ACCENT);
                    label(&painter, pane.left_top() + egui::vec2(8.0, 8.0), egui::Align2::LEFT_TOP, 0, &a.name);
                    label(&painter, pane.right_top() + egui::vec2(-8.0, 8.0), egui::Align2::RIGHT_TOP, 1, &b.name);
                }
                ([Some(a), Some(b)], Mode::Onion) => {
                    painter.image(a.texture.id(), place(a), uv, egui::Color32::WHITE);
                    painter.image(b.texture.id(), place(b), uv, egui::Color32::from_white_alpha((self.opacity * 255.0) as u8));
                    label(&painter, pane.left_top() + egui::vec2(8.0, 8.0), egui::Align2::LEFT_TOP, 0, &a.name);
                    label(&painter, pane.right_top() + egui::vec2(-8.0, 8.0), egui::Align2::RIGHT_TOP, 1, &format!("{} · {:.0} %", b.name, self.opacity * 100.0));
                }
                ([Some(_), Some(_)], Mode::Difference) => {
                    if let Some(d) = &self.difference { painter.image(d.texture.id(), canvas, uv, egui::Color32::WHITE); }
                }
                _ => {
                    // Side by side, or a single image: each pane shows one.
                    let side = if both { index } else if self.pictures[0].is_some() { 0 } else { 1 };
                    if let Some(p) = &self.pictures[side] {
                        painter.image(p.texture.id(), place(p), uv, egui::Color32::WHITE);
                        label(&painter, pane.left_top() + egui::vec2(8.0, 8.0), egui::Align2::LEFT_TOP, side, &p.name);
                    }
                    if !both {
                        painter.text(pane.center_bottom() - egui::vec2(0.0, 14.0), egui::Align2::CENTER_BOTTOM,
                            format!("Add image {} to compare", ["A", "B"][1 - side]), egui::FontId::proportional(13.0), theme::TEXT_STRONG);
                    }
                }
            }
            // Colors of both images under the pointer.
            if let Some(p) = pointer.filter(|p| pane.contains(*p) && canvas.contains(*p)) {
                let (x, y) = (((p.x - canvas.left()) / scale) as u32, ((p.y - canvas.top()) / scale) as u32);
                let hex = |side: usize| self.pictures[side].as_ref().and_then(|pic| pic.pixels.get_pixel_checked(x, y))
                    .map_or("—".to_owned(), |px| format!("#{:02X}{:02X}{:02X}{:02X}", px[0], px[1], px[2], px[3]));
                readout = Some(format!("x {x}, y {y}    A {}    B {}", hex(0), hex(1)));
            }
        }

        // Dragging (or clicking) moves the swipe divider.
        if self.mode == Mode::Swipe && both {
            if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal); }
            if let (Some(p), true) = (response.interact_pointer_pos(), response.dragged() || response.clicked()) {
                let (canvas, _) = canvas(view, size, self.zoom, self.pan);
                self.split = ((p.x - canvas.left()) / canvas.width()).clamp(0.0, 1.0);
            }
        }
        readout
    }

    fn load_sample_with(&mut self, ctx: &egui::Context) {
        self.set(ctx, 0, "afternoon.png".into(), "Sample · 480×300".into(), landscape(false));
        self.set(ctx, 1, "sunset.png".into(), "Sample · 480×300".into(), landscape(true));
        self.split = 0.5;
        self.refresh(ctx);
    }
}

impl Tool for ImageCompare {
    fn name(&self) -> &'static str { "Image Compare" }
    fn icon(&self) -> &'static str { icon::SQUARE_SPLIT_HORIZONTAL }
    fn category(&self) -> Category { Category::Compare }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let area = ui.available_rect_before_wrap();
        self.take_dropped(&ctx);
        if std::mem::take(&mut self.pending_sample) { self.load_sample_with(&ctx); }
        ui::description(ui, "Drag the divider to compare two images, or switch to side by side, onion skin or a map of the differences.");
        ui.add_space(10.0);
        self.images_toolbar(ui);
        self.view_toolbar(ui);
        ui::error_label(ui, &self.error);
        let readout = self.view(ui);
        ui.horizontal(|ui| {
            ui.set_height(STATUS_HEIGHT);
            let text = readout.unwrap_or_else(|| {
                self.pictures.iter().zip(["A", "B"]).filter_map(|(p, letter)| p.as_ref().map(|p| format!("{letter}: {}", p.info))).collect::<Vec<_>>().join("     ")
            });
            ui.label(egui::RichText::new(text).monospace().small().color(theme::SUBTLE));
        });
        ui::drop_hint(&ctx, area, "Drop one or two images to compare");
    }

    fn load_sample(&mut self) {
        // Textures need the egui context, which only `ui` receives: ask it to load the sample.
        self.pictures = [None, None];
        self.pending_sample = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_differing_pixels() {
        let a = RgbaImage::from_pixel(4, 2, image::Rgba([10, 20, 30, 255]));
        let mut b = a.clone();
        b.put_pixel(1, 1, image::Rgba([10, 20, 40, 255]));
        assert_eq!(difference(&a, &a, 0).1, 0);
        assert_eq!(difference(&a, &b, 0).1, 1);
        assert_eq!(difference(&a, &b, 10).1, 0, "within tolerance");
        // A taller image: its extra row counts as different.
        let tall = RgbaImage::from_pixel(4, 3, image::Rgba([10, 20, 30, 255]));
        let (map, changed) = difference(&a, &tall, 0);
        assert_eq!((map.dimensions(), changed), ((4, 3), 4));
    }

    #[test]
    fn sample_images_differ() {
        let (before, after) = (landscape(false), landscape(true));
        let (_, changed) = difference(&before, &after, 0);
        assert!(changed > 1000 && changed < 480 * 300 / 10, "{changed}");
    }

    #[test]
    fn decodes_png_files() {
        let mut png = Vec::new();
        RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let (info, pixels) = decode("x.png", &png).unwrap();
        assert_eq!(pixels.dimensions(), (3, 2));
        assert_eq!(info, format!("PNG · 3×2 · {} B", png.len()));
        assert!(decode("x.txt", b"hello").is_err());
    }
}
