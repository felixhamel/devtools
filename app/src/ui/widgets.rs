//! Small shared widgets: buttons, pane toolbars, the two-pane layout, the
//! segmented control, badges, error banners and empty states.
use eframe::egui;
use super::{clipboard_text, icon, theme, MUTED};

/// Text size of toolbar buttons and segmented controls, which must match so
/// that both are as tall.
const BUTTON_TEXT_SIZE: f32 = 12.5;

/// A muted one-line explanation shown under a tool's title.
pub fn description(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(MUTED));
}

/// A compact toolbar button; the primary one is mint green.
pub fn action_button(label: impl Into<String>, primary: bool) -> egui::Button<'static> {
    let (fill, text) = if primary { (theme::ACCENT_STRONG, theme::ON_ACCENT) } else { (theme::BUTTON, theme::TEXT) };
    egui::Button::new(egui::RichText::new(label.into()).size(BUTTON_TEXT_SIZE).color(text)).fill(fill).corner_radius(5)
}

/// An action button with a leading icon, e.g. `icon_button(icon::COPY, "Copy", false)`.
pub fn icon_button(glyph: &str, label: &str, primary: bool) -> egui::Button<'static> {
    action_button(format!("{glyph}  {label}"), primary)
}

/// A compact button showing only an icon; `tooltip` says what it does.
/// Returns true when clicked.
pub fn icon_only(ui: &mut egui::Ui, glyph: &str, tooltip: &str) -> bool {
    let button = egui::Button::new(egui::RichText::new(glyph).size(14.0).color(theme::TEXT))
        .fill(theme::BUTTON).corner_radius(5).min_size(egui::vec2(28.0, 0.0));
    ui.add(button).on_hover_text(tooltip).clicked()
}

/// Icon button that loads a tool's example.
pub fn sample_button(ui: &mut egui::Ui) -> bool { icon_only(ui, icon::SPARKLE, "Load an example") }

/// Icon button that empties a tool.
pub fn clear_button(ui: &mut egui::Ui) -> bool { icon_only(ui, icon::ERASER, "Clear") }

/// Pastes the clipboard's text into `target` (or puts the reason in `error`)
/// when clicked. Returns true when clicked, even if the paste failed.
pub fn paste_button(ui: &mut egui::Ui, target: &mut String, error: &mut String) -> bool {
    let clicked = icon_only(ui, icon::CLIPBOARD_TEXT, "Paste from the clipboard");
    if clicked {
        match clipboard_text() { Ok(text) => *target = text, Err(e) => *error = e }
    }
    clicked
}

/// "Copy" button that turns into "Copied" for a moment after a click.
pub fn copy_button(ui: &mut egui::Ui, text: &str) {
    copy_with_feedback(ui, text, ui.id().with("copy-feedback"), CopyStyle::Labeled);
}

/// Icon-only copy button (a check mark appears after a click). `id` must be
/// unique among the buttons shown together, e.g. the row index in a table.
pub fn copy_icon(ui: &mut egui::Ui, text: &str, id: impl std::hash::Hash) {
    copy_with_feedback(ui, text, egui::Id::new(("copy-icon", id)), CopyStyle::Icon);
}

/// How a copy button looks: "Copy" with an icon, the icon alone, or a faint
/// icon (in lists, until the row is hovered).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum CopyStyle {
    /// Icon and "Copy" / "Copied".
    Labeled,
    /// Icon on a button fill.
    Icon,
    /// Gray icon without a fill; looks like [`CopyStyle::Icon`] while "Copied" shows.
    Faint,
}

/// A button that copies `text` and shows a check mark ("Copied") for a moment.
/// The time of the last click is kept in egui's memory under `id`.
pub(super) fn copy_with_feedback(ui: &mut egui::Ui, text: &str, id: egui::Id, style: CopyStyle) {
    /// Seconds the "Copied" feedback stays.
    const FEEDBACK: f64 = 1.5;
    let now = ui.input(|i| i.time);
    // Seconds of "Copied" feedback left, if the button was clicked recently.
    let remaining = ui.data(|d| d.get_temp::<f64>(id)).map(|at| FEEDBACK - (now - at)).filter(|left| *left > 0.0);
    let (glyph, label) = if remaining.is_some() { (icon::CHECK, "Copied") } else { (icon::COPY, "Copy") };
    let button = match style {
        CopyStyle::Labeled => icon_button(glyph, label, false),
        CopyStyle::Icon | CopyStyle::Faint => {
            let faint = style == CopyStyle::Faint && remaining.is_none();
            let color = if remaining.is_some() { theme::ACCENT } else if faint { theme::SUBTLE } else { theme::TEXT };
            let fill = if faint { egui::Color32::TRANSPARENT } else { theme::BUTTON };
            egui::Button::new(egui::RichText::new(glyph).size(13.0).color(color)).fill(fill).corner_radius(5)
        }
    };
    if ui.add(button).on_hover_text("Copy to the clipboard").clicked() {
        ui.ctx().copy_text(text.to_owned());
        ui.data_mut(|d| d.insert_temp(id, now));
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(FEEDBACK));
    } else if let Some(left) = remaining {
        // Repaint once more when the feedback should disappear.
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(left));
    }
}

/// A pane header: a bold label followed by compact buttons, wrapping when narrow.
pub fn toolbar<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(9.0, 3.0);
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(label).strong().color(theme::TEXT));
            ui.add_space(4.0);
            add(ui)
        }).inner
    }).inner;
    ui.add_space(6.0);
    inner
}

/// Two equal panes side by side, filling the remaining space.
pub fn split<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut egui::Ui) -> R) -> R {
    ui.columns(2, |columns| { let (left, right) = columns.split_at_mut(1); add(&mut left[0], &mut right[0]) })
}

/// A small rounded status pill, e.g. `badge(ui, icon::CHECK_CIRCLE, "Signature verified", theme::DIFF_ADDED)`.
pub fn badge(ui: &mut egui::Ui, glyph: &str, text: &str, color: egui::Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.18)).stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.6)))
        .corner_radius(10).inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| { ui.label(egui::RichText::new(format!("{glyph}  {text}")).color(color)); });
}

/// Shows `error` in a red banner with a warning icon, or nothing when it is empty.
pub fn error_label(ui: &mut egui::Ui, error: &str) {
    if error.is_empty() { return; }
    egui::Frame::new().fill(theme::ERROR_BG).stroke(egui::Stroke::new(1.0_f32, theme::ERROR_STROKE))
        .corner_radius(6).inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(icon::WARNING_CIRCLE).color(theme::ERROR_TEXT));
                ui.label(egui::RichText::new(error).color(theme::ERROR_TEXT));
            });
        });
    ui.add_space(6.0);
}

/// Joined toggle buttons of which exactly one is selected, e.g. view modes.
/// Returns true when the user picked another option.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, current: &mut T, options: &[(T, &str)]) -> bool {
    // A frame cannot wrap by itself: in a wrapping toolbar, start a new line when it does not fit.
    if ui.layout().main_wrap {
        // Estimated width: each label with its button padding and the 2 px gap
        // between buttons, plus a few pixels for the frame's margin and border.
        let padding = 2.0 * ui.spacing().button_padding.x + 2.0;
        let width: f32 = options.iter()
            .map(|(_, label)| ui.fonts(|f| f.layout_no_wrap((*label).to_owned(), egui::FontId::proportional(BUTTON_TEXT_SIZE), theme::TEXT).size().x) + padding)
            .sum::<f32>() + 6.0;
        if width > ui.available_size_before_wrap().x { ui.end_row(); }
    }
    let mut changed = false;
    egui::Frame::new().fill(theme::SEARCH_BG).stroke(egui::Stroke::new(1.0_f32, theme::EDITOR_STROKE))
        .corner_radius(6).inner_margin(1)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            // As tall as the toolbar's other buttons.
            ui.spacing_mut().button_padding.y = 1.0;
            ui.horizontal(|ui| {
                for (value, label) in options {
                    let selected = *current == *value;
                    let text = egui::RichText::new(*label).size(BUTTON_TEXT_SIZE).color(if selected { theme::ON_ACCENT } else { theme::TEXT });
                    let fill = if selected { theme::ACCENT_STRONG } else { egui::Color32::TRANSPARENT };
                    let button = egui::Button::new(text).fill(fill).stroke(egui::Stroke::NONE).corner_radius(4);
                    if ui.add(button).clicked() && !selected {
                        *current = *value;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Space that centers content of `height` vertically in what is left of `ui`.
pub fn center_vertically(ui: &mut egui::Ui, height: f32) {
    ui.add_space(((ui.available_height() - height) / 2.0).max(0.0));
}

/// A centered icon and hint in an empty preview, e.g. before an image is loaded.
pub fn empty_state(ui: &mut egui::Ui, glyph: &str, text: &str) {
    // 40 px is about half the height of the icon and hint together.
    ui.add_space((ui.available_height() / 2.0 - 40.0).max(8.0));
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(glyph).size(34.0).color(theme::SUBTLE));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(text).color(theme::SUBTLE));
    });
}
