//! Base64 Encode/Decode for text and files, with the URL-safe alphabet and
//! optional padding; binary results are shown as a hex dump.
use base64::{engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD}, Engine};
use eframe::egui;
use super::{encoding::decode_base64, Category, Tool};
use crate::ui::{self, icon, theme, TextIo};

const SAMPLE: &str = "Hello, DevUtils! 👋";
/// Larger binary results are cut in the hex dump.
const DUMP_LIMIT: usize = 64 * 1024;

/// `00000000  48 65 6c 6c 6f 2c 20 44  65 76 55 74 69 6c 73 21  |Hello, DevUtils!|`
fn hex_dump(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (row, chunk) in bytes.chunks(16).enumerate() {
        let hex: Vec<String> = (0..16).map(|i| chunk.get(i).map_or("  ".into(), |b| format!("{b:02x}"))).collect();
        let ascii: String = chunk.iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect();
        out.push_str(&format!("{:08x}  {}  {}  |{ascii}|\n", row * 16, hex[..8].join(" "), hex[8..].join(" ")));
    }
    out
}

#[derive(Default)]
pub struct Base64String {
    io: TextIo,
    url_safe: bool,
    no_padding: bool,
    /// A dropped file to encode instead of the text: name and contents.
    file: Option<(String, Vec<u8>)>,
    /// Shown above the output when the decoded bytes are not text.
    note: String,
}

impl Base64String {
    fn engine(&self) -> &'static base64::engine::GeneralPurpose {
        match (self.url_safe, self.no_padding) {
            (false, false) => &STANDARD,
            (false, true) => &STANDARD_NO_PAD,
            (true, false) => &URL_SAFE,
            (true, true) => &URL_SAFE_NO_PAD,
        }
    }

    fn run(&mut self) {
        self.note.clear();
        if self.io.active == 0 {
            let bytes = match &self.file { Some((_, bytes)) => bytes.as_slice(), None => self.io.input.as_bytes() };
            self.io.apply(Ok(self.engine().encode(bytes)));
            return;
        }
        if self.io.input.trim().is_empty() { self.io.apply(Ok(String::new())); return; }
        match decode_base64(&self.io.input) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => self.io.apply(Ok(text)),
                Err(e) => {
                    let bytes = e.into_bytes();
                    self.note = format!("Binary data ({} bytes), shown as a hex dump{}", bytes.len(), if bytes.len() > DUMP_LIMIT { " of the first 64 KB" } else { "" });
                    self.io.apply(Ok(hex_dump(&bytes[..bytes.len().min(DUMP_LIMIT)])));
                }
            },
            Err(e) => self.io.apply(Err(e)),
        }
    }
}

impl Tool for Base64String {
    fn name(&self) -> &'static str { "Base64 String Encode/Decode" }
    fn icon(&self) -> &'static str { icon::BINARY }
    fn category(&self) -> Category { Category::Encoding }

    fn ui(&mut self, ui: &mut egui::Ui) {
        // A file dropped on the window is encoded as is.
        for file in ui.ctx().input(|i| i.raw.dropped_files.clone()) {
            let Some(path) = &file.path else { continue };
            match std::fs::read(path) {
                Ok(bytes) => {
                    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
                    self.file = Some((name, bytes));
                    self.io.active = 0;
                    self.run();
                }
                Err(e) => self.io.error = format!("Cannot read {}: {e}", path.display()),
            }
        }
        ui::description(ui, "Encode text or a file dropped on the window, or decode Base64 (line breaks, missing padding and the URL-safe alphabet are accepted).");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            if let Some((name, bytes)) = &self.file {
                let mut remove = false;
                ui::editor_frame().show(left, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("{}  {name} · {} bytes will be encoded", icon::FILE, bytes.len())).color(theme::TEXT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| remove = ui::icon_only(ui, icon::X, "Encode the text instead"));
                    });
                });
                left.add_space(6.0);
                if remove { self.file = None; self.run(); }
            }
            let input_before = self.io.input.len();
            let mut run_now = self.io.input_pane(left, &["Encode", "Decode"], "Text to encode, or Base64 to decode…", Some(SAMPLE));
            // Typing replaces a dropped file.
            if self.file.is_some() && self.io.input.len() != input_before { self.file = None; run_now = true; }
            let reuse = self.io.output_pane(right, |ui, output| {
                let reuse = super::reuse_menu(ui, output, "Base64 String Encode/Decode");
                ui.separator();
                run_now |= ui.checkbox(&mut self.url_safe, "URL-safe").on_hover_text("Use - and _ instead of + and /").changed();
                run_now |= ui.checkbox(&mut self.no_padding, "No padding").on_hover_text("Omit the trailing = characters").changed();
                reuse
            });
            if !self.note.is_empty() { right.label(egui::RichText::new(&self.note).small().color(theme::WARNING)); }
            if reuse { self.io.input = std::mem::take(&mut self.io.output); self.file = None; run_now = true; }
            if run_now { self.run(); }
        });
    }

    fn load_sample(&mut self) {
        self.io.input = SAMPLE.into();
        self.file = None;
        self.io.active = 0;
        self.run();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.io.input = text.to_owned();
        self.file = None;
        self.run();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_with_options() {
        let mut tool = Base64String { io: TextIo { input: "subjects?_d".into(), ..TextIo::default() }, ..Default::default() };
        tool.run();
        assert_eq!(tool.io.output, "c3ViamVjdHM/X2Q=");
        tool.url_safe = true;
        tool.no_padding = true;
        tool.run();
        assert_eq!(tool.io.output, "c3ViamVjdHM_X2Q");
        tool.file = Some(("a.bin".into(), vec![0xff, 0x00]));
        tool.run();
        assert_eq!(tool.io.output, "_wA");
    }

    #[test]
    fn decodes_binary_as_hex_dump() {
        let mut tool = Base64String { io: TextIo { input: "AP8QSGk=".into(), active: 1, ..TextIo::default() }, ..Default::default() };
        tool.run();
        assert!(tool.note.starts_with("Binary data (5 bytes)"), "{}", tool.note);
        // Each half of the hex column is 23 characters wide (8 bytes).
        assert_eq!(tool.io.output, format!("00000000  {:<23}  {:<23}  |...Hi|\n", "00 ff 10 48 69", ""));
        tool.io.input = "SGVsbG8".into();
        tool.run();
        assert_eq!((tool.io.output.as_str(), tool.note.as_str()), ("Hello", ""));
    }
}
