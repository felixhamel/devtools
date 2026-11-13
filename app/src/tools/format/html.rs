//! HTML beautifier (hand-written) and minifier (minify-html).

/// A piece of an HTML document. Slices borrow from the source.
#[derive(Debug)]
enum Tok<'a> {
    Open { name: String, raw: &'a str, self_closing: bool },
    Close { name: String, raw: &'a str },
    Text(&'a str),
    /// Comments, `<!DOCTYPE>` and `<?…?>`.
    Other(&'a str),
    /// Untouched contents of `script`, `style`, `pre` and `textarea`.
    Raw(&'a str),
}

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
/// Elements whose content is not HTML (or whose whitespace matters).
const RAW: &[&str] = &["script", "style", "pre", "textarea"];

fn tag_name(raw: &str) -> String {
    raw.trim_start_matches(['<', '/']).chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':')
        .collect::<String>().to_ascii_lowercase()
}

/// Byte offset of the first ASCII case-insensitive match of `needle`, without
/// allocating a lowercase copy of the haystack.
fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    haystack.as_bytes().windows(needle.len()).position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn tokens(src: &str) -> Result<Vec<Tok<'_>>, String> {
    let bytes = src.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &src[i..];
        i = if rest.starts_with("<!--") {
            let end = i + rest.find("-->").ok_or("Unterminated comment")? + 3;
            toks.push(Tok::Other(&src[i..end]));
            end
        } else if rest.starts_with("<!") || rest.starts_with("<?") {
            let end = i + rest.find('>').ok_or("Unterminated declaration")? + 1;
            toks.push(Tok::Other(&src[i..end]));
            end
        } else if rest.starts_with("</") {
            let end = i + rest.find('>').ok_or("Unterminated closing tag")? + 1;
            toks.push(Tok::Close { name: tag_name(&src[i..end]), raw: &src[i..end] });
            end
        } else if rest.starts_with('<') && bytes.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
            // Find the closing `>` while skipping quoted attribute values.
            let mut j = i + 1;
            let mut quote = None;
            while j < bytes.len() {
                match (quote, bytes[j]) {
                    (None, b'"' | b'\'') => quote = Some(bytes[j]),
                    (Some(q), b) if b == q => quote = None,
                    (None, b'>') => break,
                    _ => {}
                }
                j += 1;
            }
            if j >= bytes.len() { return Err("Unterminated tag".into()); }
            let raw = &src[i..=j];
            let name = tag_name(raw);
            let self_closing = raw.ends_with("/>");
            let mut end = j + 1;
            let is_raw = RAW.contains(&name.as_str()) && !self_closing;
            if is_raw {
                // Everything up to the matching closing tag is kept verbatim.
                let body_end = find_ascii_ci(&src[end..], &format!("</{name}")).map_or(src.len(), |k| end + k);
                toks.push(Tok::Open { name, raw, self_closing });
                if body_end > end { toks.push(Tok::Raw(&src[end..body_end])); }
                end = body_end;
            } else {
                toks.push(Tok::Open { name, raw, self_closing });
            }
            end
        } else {
            // Text up to the next `<` (a lone `<` that does not start a tag is text).
            let next = rest[1..].find('<').map_or(src.len(), |k| i + 1 + k);
            toks.push(Tok::Text(&src[i..next]));
            next
        };
    }
    Ok(toks)
}

fn collapse(text: &str) -> String { text.split_whitespace().collect::<Vec<_>>().join(" ") }

/// Puts each element on its own line, indented by nesting depth.
/// - An element whose only content is text stays on one line: `<p>Hello</p>`.
/// - `pre` and `textarea` keep their content byte for byte.
/// - `script` and `style` content is re-indented (common indentation removed).
pub(super) fn beautify(src: &str) -> Result<String, String> {
    let toks = tokens(src)?;
    let mut out: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let pad = |depth: usize| "  ".repeat(depth);
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Open { name, raw, self_closing } => {
                if *self_closing || VOID.contains(&name.as_str()) {
                    out.push(format!("{}{raw}", pad(depth)));
                    i += 1;
                    continue;
                }
                let body = match toks.get(i + 1) { Some(Tok::Raw(body)) => Some(*body), _ => None };
                // The closing tag of this element, if it comes right after the open tag (or its raw body).
                let close_at = i + 1 + usize::from(body.is_some());
                let close = match toks.get(close_at) { Some(Tok::Close { name: n, raw }) if n == name => Some(*raw), _ => None };
                let consumed_to = if close.is_some() { close_at } else { close_at - 1 };
                match body {
                    Some(body) if name == "pre" || name == "textarea" => {
                        out.push(format!("{}{raw}{body}{}", pad(depth), close.unwrap_or("")));
                        i = consumed_to;
                    }
                    Some(body) => {
                        out.push(format!("{}{raw}", pad(depth)));
                        let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
                        let common = lines.iter().map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
                        for line in lines { out.push(format!("{}{}", pad(depth + 1), line[common..].trim_end())); }
                        if let Some(close) = close { out.push(format!("{}{close}", pad(depth))); }
                        i = consumed_to;
                    }
                    None => match (toks.get(i + 1), toks.get(i + 2)) {
                        // <p>text</p> on one line.
                        (Some(Tok::Text(text)), Some(Tok::Close { name: n, raw: close })) if n == name => {
                            out.push(format!("{}{raw}{}{close}", pad(depth), collapse(text)));
                            i += 2;
                        }
                        // Empty element: <div></div>.
                        (Some(Tok::Close { name: n, raw: close }), _) if n == name => {
                            out.push(format!("{}{raw}{close}", pad(depth)));
                            i += 1;
                        }
                        _ => { out.push(format!("{}{raw}", pad(depth))); depth += 1; }
                    },
                }
            }
            Tok::Close { raw, .. } => { depth = depth.saturating_sub(1); out.push(format!("{}{raw}", pad(depth))); }
            Tok::Text(text) => { let text = collapse(text); if !text.is_empty() { out.push(format!("{}{text}", pad(depth))); } }
            Tok::Other(other) => out.push(format!("{}{}", pad(depth), other.trim())),
            Tok::Raw(body) => out.push(body.to_string()),
        }
        i += 1;
    }
    Ok(out.join("\n"))
}

/// Minifies HTML and the CSS/JS it contains. Closing tags and `<html>`/`<head>`
/// are kept so the result stays easy to read back.
pub(super) fn minify(src: &str) -> Result<String, String> {
    let mut cfg = minify_html::Cfg::new();
    cfg.minify_css = true;
    cfg.minify_js = true;
    cfg.keep_closing_tags = true;
    cfg.keep_html_and_head_opening_tags = true;
    String::from_utf8(minify_html::minify(src.as_bytes(), &cfg)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beautify_layout() {
        let src = "<!DOCTYPE html><html><head><title>Hi</title><style>\n    a { b: c }\n</style></head><body><p>Hello   world</p><br><div><span>x</span></div><div></div><pre>  keep\n   this</pre><SCRIPT>let a = '<div>';</SCRIPT></body></html>";
        assert_eq!(beautify(src).unwrap(), [
            "<!DOCTYPE html>", "<html>", "  <head>", "    <title>Hi</title>", "    <style>", "      a { b: c }", "    </style>", "  </head>",
            "  <body>", "    <p>Hello world</p>", "    <br>", "    <div>", "      <span>x</span>", "    </div>", "    <div></div>",
            "    <pre>  keep\n   this</pre>", "    <SCRIPT>", "      let a = '<div>';", "    </SCRIPT>", "  </body>", "</html>",
        ].join("\n"));
        assert!(beautify("<a href=\"x").is_err());
    }

    #[test]
    fn case_insensitive_search() {
        assert_eq!(find_ascii_ci("abc</ScRiPt>", "</script"), Some(3));
        assert_eq!(find_ascii_ci("abc", "</script"), None);
    }

    #[test]
    fn minify_shrinks() {
        let out = minify("<div>\n   <p>  Hello  </p>\n</div>").unwrap();
        assert!(out.len() < 25, "{out}");
        assert!(out.contains("Hello"));
    }
}
