//! Transient feedback drawn over the window: toasts and the "drop here" hint
//! shown while files are dragged over a tool.
use eframe::egui;
use super::{icon, theme};

/// While files are dragged over the window, covers `rect` with a dashed
/// "drop here" frame and `text`.
pub fn drop_hint(ctx: &egui::Context, rect: egui::Rect, text: &str) {
    if ctx.input(|i| i.raw.hovered_files.is_empty()) { return; }
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop-hint")));
    painter.rect_filled(rect, 10.0, egui::Color32::from_black_alpha(190));
    let r = rect.shrink(8.0);
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    painter.extend(egui::Shape::dashed_line(&corners, egui::Stroke::new(2.0_f32, theme::ACCENT), 10.0, 7.0));
    painter.text(rect.center() - egui::vec2(0.0, 18.0), egui::Align2::CENTER_CENTER, icon::DOWNLOAD_SIMPLE, egui::FontId::proportional(42.0), theme::ACCENT);
    painter.text(rect.center() + egui::vec2(0.0, 24.0), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(16.0), theme::TEXT_STRONG);
}

/// A message shown for a moment at the bottom of the window.
#[derive(Clone)]
struct Toast {
    /// Icon shown before the text, e.g. `icon::STAR`.
    glyph: &'static str,
    text: String,
    /// `ctx` time (seconds) when it was posted.
    at: f64,
}

/// Where the current [`Toast`] is kept in egui's memory (there is at most one).
fn toast_id() -> egui::Id { egui::Id::new("devutils-toast") }

/// Shows `text` at the bottom of the window for a few seconds (it replaces
/// the previous message, if any).
pub fn toast(ctx: &egui::Context, glyph: &'static str, text: impl Into<String>) {
    let at = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(toast_id(), Toast { glyph, text: text.into(), at }));
}

/// Draws the current toast, sliding it in and fading it out. Called once per frame by the app.
pub fn show_toast(ctx: &egui::Context) {
    /// Seconds a toast stays, fades included.
    const SHOWN: f64 = 2.8;
    /// Seconds of the fade in and of the fade out.
    const FADE: f64 = 0.25;
    let Some(toast) = ctx.data(|d| d.get_temp::<Toast>(toast_id())) else { return };
    let age = ctx.input(|i| i.time) - toast.at;
    if age > SHOWN {
        ctx.data_mut(|d| d.remove::<Toast>(toast_id()));
        return;
    }
    // Opacity ramps up over the first FADE seconds and down over the last ones;
    // meanwhile the toast rises 14 px into place.
    let opacity = (age / FADE).min((SHOWN - age) / FADE).clamp(0.0, 1.0) as f32;
    let rise = (1.0 - (age / FADE).min(1.0) as f32) * 14.0;
    egui::Area::new(toast_id().with("area")).order(egui::Order::Tooltip).interactable(false)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -28.0 + rise))
        .show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            egui::Frame::new().fill(theme::TOAST_BG).stroke(egui::Stroke::new(1.0_f32, theme::ACCENT_DARK))
                .corner_radius(18).inner_margin(egui::Margin::symmetric(16, 9))
                .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: egui::Color32::from_black_alpha(110) })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(toast.glyph).size(15.0).color(theme::ACCENT));
                        ui.label(egui::RichText::new(&toast.text).color(theme::TEXT_STRONG));
                    });
                });
        });
    ctx.request_repaint();
}
