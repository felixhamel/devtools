//! The look and the shared building blocks of every tool.
//!
//! Tools use these through `ui::…` (e.g. `ui::toolbar`, `ui::theme::ACCENT`);
//! the submodules only group them by role:
//!
//! - [`theme`]: every color; [`style`]: egui visuals and fonts;
//! - [`widgets`]: buttons, toolbars, the two-pane layout, segmented control, badges;
//! - [`editor`]: text editors and previews with highlighting;
//! - [`properties`](mod@properties): `Key: value` reports with a copy button per value;
//! - [`text_io`]: the live input/output state of text tools;
//! - [`files`]: Open dialog, dropped files, clipboard;
//! - [`feedback`]: toasts and the "drop here" hint.

/// Phosphor icons (regular weight), e.g. `icon::COPY`. They are glyphs of the
/// proportional and monospace fonts, so they can go in any label or button.
pub use egui_phosphor::regular as icon;

pub mod theme;
mod editor;
mod feedback;
mod files;
mod properties;
mod style;
mod text_io;
mod widgets;

pub use editor::*;
pub use feedback::*;
pub use files::*;
pub use properties::*;
pub use style::*;
pub use text_io::*;
// The usual color of secondary labels, short enough to use as `ui::MUTED`.
pub use theme::MUTED;
pub use widgets::*;
