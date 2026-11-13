//! CSS and SCSS beautifier / minifier.
//!
//! Works on a small token stream rather than a full parser, which keeps comments
//! and handles SCSS (nesting, `//` comments, `#{…}` interpolation) the same way.

#[derive(Debug, PartialEq)]
enum Tok { Text(String), Space, Comment(String), Open, Close, Semi }

/// Splits CSS/SCSS into the pieces that drive layout. Strings, the contents of
/// parentheses (e.g. `url(data:…;…)`) and SCSS `#{…}` stay inside `Text`, so the
/// braces and semicolons they contain are not mistaken for structure.
fn tokens(src: &str, scss: bool) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut text = String::new();
    let mut paren = 0usize;
    let mut i = 0;
    let flush = |text: &mut String, toks: &mut Vec<Tok>| if !text.is_empty() { toks.push(Tok::Text(std::mem::take(text))); };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '"' | '\'' => {
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
                i = end + 1;
            }
            // In plain CSS `//` is not a comment (it appears in `url(http://…)`).
            '/' if scss && next == Some('/') && paren == 0 => {
                flush(&mut text, &mut toks);
                let end = (i..chars.len()).find(|&j| chars[j] == '\n').unwrap_or(chars.len());
                toks.push(Tok::Comment(chars[i..end].iter().collect::<String>().trim_end().to_string()));
                i = end;
                continue;
            }
            '#' if scss && next == Some('{') => {
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
pub(super) fn beautify(src: &str, scss: bool) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut depth = 0usize;
    let emit = |out: &mut Vec<String>, depth: usize, s: &str| out.push(format!("{}{s}", "  ".repeat(depth)));
    for tok in tokens(src, scss)? {
        match tok {
            Tok::Text(s) => line.push_str(&s),
            Tok::Space => if !line.is_empty() && !line.ends_with(' ') { line.push(' ') },
            Tok::Comment(c) => {
                if !line.trim().is_empty() { emit(&mut out, depth, line.trim()); }
                line.clear();
                emit(&mut out, depth, &c);
            }
            Tok::Open => {
                emit(&mut out, depth, format!("{} {{", line.trim()).trim_start());
                line.clear();
                depth += 1;
            }
            Tok::Semi => {
                if !line.trim().is_empty() { emit(&mut out, depth, &format!("{};", line.trim())); }
                line.clear();
            }
            Tok::Close => {
                if !line.trim().is_empty() { emit(&mut out, depth, &format!("{};", line.trim())); }
                line.clear();
                depth = depth.checked_sub(1).ok_or("Unexpected }")?;
                emit(&mut out, depth, "}");
                if depth == 0 { out.push(String::new()); }
            }
        }
    }
    if depth != 0 { return Err("Missing }".into()); }
    if !line.trim().is_empty() { emit(&mut out, 0, line.trim()); }
    while out.last().is_some_and(String::is_empty) { out.pop(); }
    Ok(out.join("\n"))
}

/// Drops comments and collapses whitespace. Spaces are removed around `{ } ;`
/// and after `:` and `,`, which never changes meaning; a space *before* `:` is
/// kept because `a :hover` and `a:hover` are different selectors.
pub(super) fn minify(src: &str, scss: bool) -> Result<String, String> {
    let mut out = String::new();
    let mut space = false;
    for tok in tokens(src, scss)? {
        match tok {
            Tok::Space | Tok::Comment(_) => space = true,
            Tok::Text(s) => {
                if space && !out.is_empty() && !out.ends_with(['{', '}', ';', ':', ',']) { out.push(' '); }
                out.push_str(&s);
                space = false;
            }
            Tok::Open => { out.push('{'); space = false; }
            Tok::Semi => { if !out.ends_with(['{', ';']) { out.push(';'); } space = false; }
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
        assert_eq!(beautify(src, false).unwrap(),
            "a,b {\n  color:red;\n  margin:0 auto;\n}\n\n/* note */\n@media (max-width: 10px) {\n  .x {\n    background:url(data:a;b);\n  }\n}");
        assert_eq!(minify(&beautify(src, false).unwrap(), false).unwrap(),
            "a,b{color:red;margin:0 auto}@media (max-width:10px){.x{background:url(data:a;b)}}");
        assert_eq!(minify("a { content: \"x ; }\" ; }", false).unwrap(), "a{content:\"x ; }\"}");
        assert!(beautify("a {", false).is_err());
        assert!(beautify("}", false).is_err());
    }

    #[test]
    fn scss_comments_nesting_and_interpolation() {
        let src = "// top\n$w: 10px;\n.a { &:hover { width: #{$w}; } // inline\n}";
        assert_eq!(beautify(src, true).unwrap(),
            "// top\n$w: 10px;\n.a {\n  &:hover {\n    width: #{$w};\n  }\n  // inline\n}");
        assert_eq!(minify(src, true).unwrap(), "$w:10px;.a{&:hover{width:#{$w}}}");
        assert_eq!(minify("a { b: url(http://x) }", false).unwrap(), "a{b:url(http://x)}");
    }
}
