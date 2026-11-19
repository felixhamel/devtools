//! Text helpers shared by several tools: secure random bytes, word splitting
//! and capitalization.

/// `N` bytes from the operating system's secure random number generator.
pub fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|e| format!("Random generator unavailable: {e}"))?;
    Ok(bytes)
}

/// Splits an identifier or phrase into lowercase words. Handles `camelCase`,
/// `PascalCase`, `snake_case`, `kebab-case`, spaces and acronyms
/// (`HTTPServer` → `http`, `server`). Digits stay attached to the word before them.
pub fn words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !current.is_empty() { words.push(std::mem::take(&mut current)); }
            continue;
        }
        if c.is_uppercase() && !current.is_empty() {
            let prev = chars[i - 1];
            let next_is_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            // New word at "aB" (camel hump) or at the last capital of an acronym ("HTTPServer" → "HTTP" | "Server").
            if prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next_is_lower) {
                words.push(std::mem::take(&mut current));
            }
        }
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() { words.push(current); }
    words
}

/// `word` with its first character in uppercase; the rest is unchanged.
pub fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|first| first.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words() {
        assert_eq!(words("helloWorld"), ["hello", "world"]);
        assert_eq!(words("HTTPServer_id-2x"), ["http", "server", "id", "2x"]);
        assert_eq!(words("user ID v2Beta"), ["user", "id", "v2", "beta"]);
        assert_eq!(words(" -_ "), Vec::<String>::new());
    }

    #[test]
    fn random_bytes_differ() {
        assert_ne!(random_bytes::<16>().unwrap(), random_bytes::<16>().unwrap());
    }
}
