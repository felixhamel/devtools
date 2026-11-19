//! Files and the clipboard: the Open dialog, dropped files, reading files and
//! clipboard text, and file sizes for people.
use eframe::egui;

/// Extensions of the image formats the app decodes (the `image` crate is
/// built with PNG, JPEG, WebP and BMP support only).
pub const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];

/// Reads text from the system clipboard. egui only exposes clipboard text through
/// paste events, so the "Clipboard" buttons go through `arboard` directly.
pub fn clipboard_text() -> Result<String, String> {
    arboard::Clipboard::new().and_then(|mut c| c.get_text()).map_err(|e| format!("Clipboard: {e}"))
}

/// Shows the system Open dialog (blocking) and returns the chosen files.
/// `extensions` limits the choice to those types; empty means any file.
pub fn pick_files(title: &str, extensions: &[&str]) -> Vec<std::path::PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if !extensions.is_empty() { dialog = dialog.add_filter("Supported files", extensions); }
    dialog.pick_files().unwrap_or_default()
}

/// Name and bytes of a file dropped on the window.
pub fn read_dropped(file: &egui::DroppedFile) -> Result<(String, Vec<u8>), String> {
    match (&file.path, &file.bytes) {
        (Some(path), _) => read_path(path),
        (None, Some(bytes)) => Ok((file.name.clone(), bytes.to_vec())),
        _ => Err(format!("Cannot read {}", file.name)),
    }
}

/// Name and bytes of a file on disk.
pub fn read_path(path: &std::path::Path) -> Result<(String, Vec<u8>), String> {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned());
    std::fs::read(path).map(|bytes| (name, bytes)).map_err(|e| format!("Cannot read {}: {e}", path.display()))
}

/// A byte count for people: "512 B", "2.4 KB", "3.1 MB".
pub fn human_size(bytes: usize) -> String {
    if bytes < 1024 { return format!("{bytes} B"); }
    let mut value = bytes as f64 / 1024.0;
    for unit in ["KB", "MB"] {
        // Move up a unit when the value would round to "1024.0".
        if value < 1023.95 { return format!("{value:.1} {unit}"); }
        value /= 1024.0;
    }
    format!("{value:.1} GB")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_for_people() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2496), "2.4 KB");
        // Just under 1 MB rounds up to the next unit instead of showing "1024.0 KB".
        assert_eq!(human_size(1_048_575), "1.0 MB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn reads_files_by_path() {
        let path = std::env::temp_dir().join(format!("devutils-test-{}.txt", std::process::id()));
        std::fs::write(&path, "héllo").unwrap();
        let read = read_path(&path);
        std::fs::remove_file(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(read, Ok((name, "héllo".as_bytes().to_vec())));
        // A missing file names the path in the error.
        let missing = std::env::temp_dir().join("devutils-missing-file.txt");
        assert!(read_path(&missing).unwrap_err().starts_with(&format!("Cannot read {}", missing.display())));
    }
}
