//! Every color the app uses, in one place: the dark window chrome, the mint
//! accent, text shades, status colors and one color per tool category.
use eframe::egui::Color32;

/// Background of the window and the central panel.
pub const WINDOW_BG: Color32 = Color32::from_rgb(46, 44, 44);
/// Background of the sidebar, a shade darker than the window.
pub const SIDEBAR_BG: Color32 = Color32::from_rgb(37, 37, 39);
/// Fill of the sidebar search box and of segmented controls.
pub const SEARCH_BG: Color32 = Color32::from_rgb(31, 33, 32);
/// Border of the sidebar search box and of the ⌘K palette.
pub const SEARCH_STROKE: Color32 = Color32::from_rgb(72, 80, 75);
/// Magnifying glass of the sidebar search box.
pub const SEARCH_ICON: Color32 = Color32::from_rgb(151, 164, 155);
/// Tool icons of the sidebar and title-bar buttons, when not selected.
pub const ICON: Color32 = Color32::from_rgb(180, 180, 182);
/// Fill of a hovered row (sidebar, property reports).
pub const ROW_HOVER: Color32 = Color32::from_rgb(48, 48, 50);
/// Mint-tinted fill of the selected sidebar row.
pub const ROW_SELECTED: Color32 = Color32::from_rgb(45, 66, 56);
/// Mint green used for icons, primary buttons and checked boxes.
pub const ACCENT: Color32 = Color32::from_rgb(130, 220, 169);
/// Slightly darker mint: fill of primary buttons and selected segments.
pub const ACCENT_STRONG: Color32 = Color32::from_rgb(119, 201, 153);
/// Dark mint: selection background and accent borders.
pub const ACCENT_DARK: Color32 = Color32::from_rgb(76, 151, 110);
/// Text drawn on an [`ACCENT_STRONG`] fill.
pub const ON_ACCENT: Color32 = Color32::from_rgb(20, 38, 28);
/// Fill of ordinary (non-primary) buttons.
pub const BUTTON: Color32 = Color32::from_rgb(70, 70, 72);
/// Fill of a hovered title-bar button.
pub const BUTTON_HOVER: Color32 = Color32::from_rgb(62, 62, 64);
/// Fill of the tool-name pill in the title bar.
pub const PILL: Color32 = Color32::from_rgb(58, 58, 58);
/// Background of editors, previews and single-line fields.
pub const EDITOR_BG: Color32 = Color32::from_rgb(31, 31, 31);
/// Border of editors, previews and other framed boxes.
pub const EDITOR_STROKE: Color32 = Color32::from_rgb(58, 58, 58);
/// Default text.
pub const TEXT: Color32 = Color32::from_rgb(225, 225, 225);
/// Emphasized text: titles, values, the selected row.
pub const TEXT_STRONG: Color32 = Color32::from_rgb(240, 240, 240);
/// Secondary text: descriptions, keys of `Key: value` reports.
pub const MUTED: Color32 = Color32::from_rgb(166, 170, 169);
/// Faint text: hints, counters, section labels.
pub const SUBTLE: Color32 = Color32::from_rgb(128, 128, 128);
/// Fill of error banners.
pub const ERROR_BG: Color32 = Color32::from_rgb(66, 36, 36);
/// Border of error banners.
pub const ERROR_STROKE: Color32 = Color32::from_rgb(112, 52, 52);
/// Text and icon of error banners.
pub const ERROR_TEXT: Color32 = Color32::from_rgb(255, 170, 160);
/// Background of highlighted ranges (e.g. regex matches).
pub const MATCH_BG: Color32 = Color32::from_rgb(40, 92, 66);
/// Added lines, verified signatures and other good news.
pub const DIFF_ADDED: Color32 = Color32::from_rgb(130, 220, 169);
/// Removed lines, invalid signatures, expired tokens and other bad news.
pub const DIFF_REMOVED: Color32 = Color32::from_rgb(255, 135, 125);
/// Warnings and in-between states: special characters, overlapping
/// subnets, a "Fair" password, a token not valid yet.
pub const WARNING: Color32 = Color32::from_rgb(240, 185, 95);
/// Line-number gutter of the compare views.
pub const GUTTER: Color32 = Color32::from_rgb(36, 36, 37);
/// Bar standing for folded (unchanged) lines in the compare views.
pub const FOLD: Color32 = Color32::from_rgb(40, 45, 52);
/// Background of the missing side of a line in the side-by-side compare view.
pub const FILLER: Color32 = Color32::from_rgb(26, 26, 26);
/// Hot pink for what must stand out: pixels that differ in the image compare,
/// the chosen split in the subnet calculator.
pub const HOT: Color32 = Color32::from_rgb(255, 64, 129);
/// Fill of the toast shown at the bottom of the window.
pub const TOAST_BG: Color32 = Color32::from_rgb(52, 56, 54);
/// Tool cards on the home screen.
pub const CARD: Color32 = Color32::from_rgb(53, 51, 51);
/// A hovered tool card on the home screen.
pub const CARD_HOVER: Color32 = Color32::from_rgb(62, 61, 61);
/// Background of the ⌘K palette.
pub const PALETTE_BG: Color32 = Color32::from_rgb(40, 40, 42);

// One color per sidebar category (icons on the home screen and in the
// palette), see `Category::color`. Some also serve elsewhere.

/// Date & scheduling; also the favorites star.
pub const AMBER: Color32 = Color32::from_rgb(240, 185, 95);
/// Data conversion; also the first image in the image compare.
pub const BLUE: Color32 = Color32::from_rgb(120, 170, 255);
/// Encoding.
pub const VIOLET: Color32 = Color32::from_rgb(180, 150, 255);
/// Security & identifiers.
pub const ROSE: Color32 = Color32::from_rgb(255, 135, 125);
/// Compare.
pub const PINK: Color32 = Color32::from_rgb(255, 120, 190);
/// Code & markup.
pub const CYAN: Color32 = Color32::from_rgb(110, 210, 230);
/// Network.
pub const LIME: Color32 = Color32::from_rgb(185, 225, 110);
