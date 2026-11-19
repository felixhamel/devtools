//! Syntax highlighting languages offered by Code Compare, and how the
//! language is guessed from a file name or the text itself.

/// Highlighting choices: (label, syntect language name or extension); "" is plain text.
pub(super) const LANGUAGES: [(&str, &str); 18] = [
    ("Plain text", ""), ("JSON", "json"), ("JavaScript / TypeScript", "js"), ("Rust", "rs"), ("Python", "py"),
    ("Go", "go"), ("Java / Kotlin", "java"), ("C / C++", "cpp"), ("C#", "cs"), ("PHP", "php"), ("Ruby", "rb"),
    ("Shell", "sh"), ("SQL", "sql"), ("HTML", "html"), ("CSS", "css"), ("XML", "xml"), ("YAML", "yaml"), ("Markdown", "md"),
];

/// The menu label of a [`LANGUAGES`] value, e.g. `"rs"` → "Rust"; "Plain text" when unknown.
pub(super) fn language_label(language: &str) -> &'static str {
    LANGUAGES.iter().find(|(_, l)| *l == language).map_or("Plain text", |(label, _)| label)
}

/// The language of a file name's extension, if it is one of [`LANGUAGES`].
pub(super) fn language_of_file(name: &str) -> Option<&'static str> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "json" => "json",
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" => "js",
        "rs" => "rs",
        "py" => "py",
        "go" => "go",
        "java" | "kt" | "kts" => "java",
        "c" | "h" | "cc" | "cpp" | "hpp" | "m" | "mm" => "cpp",
        "cs" => "cs",
        "php" => "php",
        "rb" => "rb",
        "sh" | "bash" | "zsh" => "sh",
        "sql" => "sql",
        "html" | "htm" | "vue" | "svelte" => "html",
        "css" | "scss" | "less" => "css",
        "xml" | "svg" | "plist" => "xml",
        "yml" | "yaml" => "yaml",
        "md" | "markdown" => "md",
        _ => return None,
    })
}

/// Guesses the language of a text from a few unmistakable cues; plain text otherwise.
pub(super) fn sniff_language(text: &str) -> &'static str {
    let start = text.trim_start();
    let first_word = start.split_whitespace().next().unwrap_or_default().to_ascii_uppercase();
    if (start.starts_with('{') || start.starts_with('[')) && serde_json::from_str::<serde_json::Value>(text).is_ok() { "json" }
    else if start.starts_with("<?xml") { "xml" }
    else if start.starts_with('<') { "html" }
    else if start.starts_with("#!") && start.lines().next().is_some_and(|line| line.contains("sh")) { "sh" }
    else if ["SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "WITH"].contains(&first_word.as_str()) { "sql" }
    else if text.contains("fn ") && (text.contains("let ") || text.contains("->")) { "rs" }
    else if text.contains("def ") && text.contains("):") { "py" }
    else if text.contains("package ") && text.contains("func ") { "go" }
    else if ["function ", "const ", "let ", "=>", "import ", "export "].iter().any(|cue| text.contains(cue)) { "js" }
    else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_languages() {
        assert_eq!(language_of_file("src/App.tsx"), Some("js"));
        assert_eq!(language_of_file("Cargo.lock"), None);
        assert_eq!(sniff_language("{\"a\": 1}"), "json");
        assert_eq!(sniff_language("select * from users"), "sql");
        assert_eq!(sniff_language("fn main() {\n    let x = 1;\n}"), "rs");
        assert_eq!(sniff_language(super::super::SAMPLE_MODIFIED), "js");
        assert_eq!(sniff_language("just some words"), "");
    }
}
