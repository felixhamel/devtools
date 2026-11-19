//! CSS and SCSS beautifier / minifier.
//!
//! Works on a small token stream rather than a full parser, which keeps comments
//! and handles SCSS (nesting, `//` comments, `#{…}` interpolation). The SCSS
//! rules are safe for plain CSS: `//` only counts as a comment outside strings
//! and parentheses (so `url(http://…)` is untouched), and `#{` never occurs in CSS.

/// A piece of CSS/SCSS, as far as layout is concerned.
#[derive(Debug, PartialEq)]
enum Tok {
    /// Selector, property or value text. Strings and `#{…}` are kept whole, and
    /// the `{ } ;` inside `(…)` stay text.
    Text(String),
    /// One or more whitespace characters.
    Space,
    /// A `/* … */` or SCSS `// …` comment, as written.
    Comment(String),
    /// `{`
    Open,
    /// `}`
    Close,
    /// `;`
    Semi,
}

/// Splits CSS/SCSS into the pieces that drive layout. Strings, the contents of
/// parentheses (e.g. `url(data:…;…)`) and SCSS `#{…}` stay inside `Text`, so the
/// braces and semicolons they contain are not mistaken for structure.
fn tokens(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut text = String::new();
    // Nesting depth of `(…)`: inside, `{ } ;` and `//` are plain text.
    let mut paren = 0usize;
    let mut i = 0;
    let flush = |text: &mut String, toks: &mut Vec<Tok>| if !text.is_empty() { toks.push(Tok::Text(std::mem::take(text))); };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '"' | '\'' => {
                // Up to the matching quote; a backslash skips the next character.
                let start = i;
                i += 1;
                while i < chars.len() && chars[i] != c { if chars[i] == '\\' { i += 1; } i += 1; }
                if i >= chars.len() { return Err("Unterminated string".into()); }
                text.extend(&chars[start..=i]);
            }
            '/' if next == Some('*') => {
                flush(&mut text, &mut toks);
                let end = (i + 2..chars.len().saturating_sub(1)).find(|&j| chars[j] == '*' && chars[j + 1] == '/').ok_or("Unterminated comment")?;
                toks.push(Tok::Comment(chars[i..end + 2].iter().collect()));
                // On the closing `/`; the `i += 1` below moves past it.
                i = end + 1;
            }
            '/' if next == Some('/') && paren == 0 => {
                flush(&mut text, &mut toks);
                let end = (i..chars.len()).find(|&j| chars[j] == '\n').unwrap_or(chars.len());
                toks.push(Tok::Comment(chars[i..end].iter().collect::<String>().trim_end().to_string()));
                // The line break is left to become a `Space`.
                i = end;
                continue;
            }
            '#' if next == Some('{') => {
                // SCSS interpolation, up to its matching `}` (it may contain braces).
                let start = i;
                let mut depth = 0;
                while i < chars.len() {
                    match chars[i] { '{' => depth += 1, '}' => { depth -= 1; if depth == 0 { break; } } _ => {} }
                    i += 1;
                }
                if i >= chars.len() { return Err("Unterminated #{…} interpolation".into()); }
                text.extend(&chars[start..=i]);
            }
            '(' => { paren += 1; text.push(c); }
            ')' => { paren = paren.saturating_sub(1); text.push(c); }
            '{' if paren == 0 => { flush(&mut text, &mut toks); toks.push(Tok::Open); }
            '}' if paren == 0 => { flush(&mut text, &mut toks); toks.push(Tok::Close); }
            ';' if paren == 0 => { flush(&mut text, &mut toks); toks.push(Tok::Semi); }
            c if c.is_whitespace() => { flush(&mut text, &mut toks); if toks.last() != Some(&Tok::Space) { toks.push(Tok::Space); } }
            _ => text.push(c),
        }
        i += 1;
    }
    flush(&mut text, &mut toks);
    Ok(toks)
}

/// One declaration or selector per line, 2-space indentation, a blank line
/// between top-level blocks. A missing final `;` before `}` is added.
pub(super) fn beautify(src: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    // The selector or declaration being collected, until `{`, `;` or `}` ends it.
    let mut line = String::new();
    let mut depth = 0usize;
    // Comments met inside a selector or declaration, written once it ends so
    // that its `;` stays on it.
    let mut held: Vec<String> = Vec::new();
    let emit = |out: &mut Vec<String>, depth: usize, s: &str| out.push(format!("{}{s}", "  ".repeat(depth)));
    let release = |out: &mut Vec<String>, held: &mut Vec<String>, depth: usize| for c in held.drain(..) { emit(out, depth, &c) };
    for tok in tokens(src)? {
        match tok {
            Tok::Text(s) => line.push_str(&s),
            Tok::Space => if !line.is_empty() && !line.ends_with(' ') { line.push(' ') },
            Tok::Comment(c) if line.trim().is_empty() => emit(&mut out, depth, &c),
            Tok::Comment(c) => held.push(c),
            Tok::Open => {
                emit(&mut out, depth, format!("{} {{", line.trim()).trim_start());
                line.clear();
                depth += 1;
                release(&mut out, &mut held, depth);
            }
            Tok::Semi => {
                if !line.trim().is_empty() { emit(&mut out, depth, &format!("{};", line.trim())); }
                line.clear();
                release(&mut out, &mut held, depth);
            }
            Tok::Close => {
                if !line.trim().is_empty() { emit(&mut out, depth, &format!("{};", line.trim())); }
                line.clear();
                release(&mut out, &mut held, depth);
                depth = depth.checked_sub(1).ok_or("Unexpected }")?;
                emit(&mut out, depth, "}");
                if depth == 0 { out.push(String::new()); }
            }
        }
    }
    if depth != 0 { return Err("Missing }".into()); }
    if !line.trim().is_empty() { emit(&mut out, 0, line.trim()); }
    release(&mut out, &mut held, 0);
    while out.last().is_some_and(String::is_empty) { out.pop(); }
    Ok(out.join("\n"))
}

/// Drops comments and collapses whitespace. Spaces are removed around `{ } ;`
/// and after `:` and `,`, which never changes meaning; a space *before* `:` is
/// kept because `a :hover` and `a:hover` are different selectors.
pub(super) fn minify(src: &str) -> Result<String, String> {
    let mut out = String::new();
    // Whitespace or a comment was skipped; a space is written only if the next text needs one.
    let mut space = false;
    for tok in tokens(src)? {
        match tok {
            Tok::Space | Tok::Comment(_) => space = true,
            Tok::Text(s) => {
                if space && !out.is_empty() && !out.ends_with(['{', '}', ';', ':', ',']) { out.push(' '); }
                out.push_str(&s);
                space = false;
            }
            Tok::Open => { out.push('{'); space = false; }
            Tok::Semi => { if !out.ends_with(['{', ';']) { out.push(';'); } space = false; }
            // The last declaration of a block needs no `;`.
            Tok::Close => { if out.ends_with(';') { out.pop(); } out.push('}'); space = false; }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_beautify_and_minify() {
        let src = "a,b{color:red;margin:0 auto}/* note */\n@media (max-width: 10px){.x{background:url(data:a;b)}}";
        assert_eq!(beautify(src).unwrap(),
            "a,b {\n  color:red;\n  margin:0 auto;\n}\n\n/* note */\n@media (max-width: 10px) {\n  .x {\n    background:url(data:a;b);\n  }\n}");
        assert_eq!(minify(&beautify(src).unwrap()).unwrap(),
            "a,b{color:red;margin:0 auto}@media (max-width:10px){.x{background:url(data:a;b)}}");
        assert_eq!(minify("a { content: \"x ; }\" ; }").unwrap(), "a{content:\"x ; }\"}");
        assert!(beautify("a {").is_err());
        assert!(beautify("}").is_err());
    }

    #[test]
    fn scss_comments_nesting_and_interpolation() {
        let src = "// top\n$w: 10px;\n.a { &:hover { width: #{$w}; } // inline\n}";
        assert_eq!(beautify(src).unwrap(),
            "// top\n$w: 10px;\n.a {\n  &:hover {\n    width: #{$w};\n  }\n  // inline\n}");
        assert_eq!(minify(src).unwrap(), "$w:10px;.a{&:hover{width:#{$w}}}");
        assert_eq!(minify("a { b: url(http://x) }").unwrap(), "a{b:url(http://x)}");
    }

    #[test]
    fn comments_inside_declarations_keep_the_semicolon() {
        assert_eq!(beautify("a { color: red /* x */; margin: 0 }").unwrap(), "a {\n  color: red;\n  /* x */\n  margin: 0;\n}");
        assert_eq!(beautify("a /* hover */ { b: 1 }").unwrap(), "a {\n  /* hover */\n  b: 1;\n}");
    }
}
