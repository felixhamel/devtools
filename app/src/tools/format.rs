//! Beautify / minify tools for code and markup.
use super::{Category, TextTool, Tool};

pub fn html() -> Box<dyn Tool> {
    Box::new(TextTool::new("HTML Beautify/Minify", "HT", Category::Code, vec![("Beautify", html_beautify), ("Minify", html_minify)]))
}

pub fn css() -> Box<dyn Tool> {
    Box::new(TextTool::new("CSS Beautify/Minify", "CS", Category::Code, vec![("Beautify", |s| css_beautify(s, false)), ("Minify", |s| css_minify(s, false))]))
}

pub fn scss() -> Box<dyn Tool> {
    Box::new(TextTool::new("SCSS Beautify/Minify", "SC", Category::Code, vec![("Beautify", |s| css_beautify(s, true)), ("Minify", |s| css_minify(s, true))]))
}

pub fn js() -> Box<dyn Tool> {
    Box::new(TextTool::new("JS Beautify/Minify", "JS", Category::Code, vec![("Beautify", |s| js_rewrite(s, false)), ("Minify", |s| js_rewrite(s, true))])
        .describe("Reprints JavaScript (JSX allowed). Beautify normalizes quotes and spacing; Minify also shortens names."))
}

pub fn xml() -> Box<dyn Tool> {
    Box::new(TextTool::new("XML Beautify/Minify", "XM", Category::Code, vec![("Beautify", |s| xml_rewrite(s, true)), ("Minify", |s| xml_rewrite(s, false))]))
}

pub fn sql() -> Box<dyn Tool> {
    Box::new(TextTool::new("SQL Formatter", "SQ", Category::Code, vec![("Format", sql_format)]))
}

fn sql_format(src: &str) -> Result<String, String> {
    Ok(sqlformat::format(src, &sqlformat::QueryParams::None, &sqlformat::FormatOptions { uppercase: Some(true), ..Default::default() }))
}

// ---------- CSS / SCSS ----------

#[derive(Debug, PartialEq)]
enum CssTok { Text(String), Space, Comment(String), Open, Close, Semi }

/// Splits CSS/SCSS into the pieces that drive layout. Strings, `url(…)` contents and
/// SCSS `#{…}` interpolation stay inside text so their braces and semicolons are ignored.
fn css_tokens(src: &str, scss: bool) -> Result<Vec<CssTok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut text = String::new();
    let mut paren = 0usize;
    let mut i = 0;
    let flush = |text: &mut String, toks: &mut Vec<CssTok>| if !text.is_empty() { toks.push(CssTok::Text(std::mem::take(text))); };
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
                toks.push(CssTok::Comment(chars[i..end + 2].iter().collect()));
                i = end + 1;
            }
            '/' if scss && next == Some('/') && paren == 0 => {
                flush(&mut text, &mut toks);
                let end = (i..chars.len()).find(|&j| chars[j] == '\n').unwrap_or(chars.len());
                toks.push(CssTok::Comment(chars[i..end].iter().collect::<String>().trim_end().to_string()));
                i = end;
                continue;
            }
            '#' if scss && next == Some('{') => {
                let mut depth = 0;
                let start = i;
                while i < chars.len() {
                    if chars[i] == '{' { depth += 1; } else if chars[i] == '}' { depth -= 1; if depth == 0 { break; } }
                    i += 1;
                }
                if i >= chars.len() { return Err("Unterminated #{…} interpolation".into()); }
                text.extend(&chars[start..=i]);
            }
            '(' => { paren += 1; text.push(c); }
            ')' => { paren = paren.saturating_sub(1); text.push(c); }
            '{' if paren == 0 => { flush(&mut text, &mut toks); toks.push(CssTok::Open); }
            '}' if paren == 0 => { flush(&mut text, &mut toks); toks.push(CssTok::Close); }
            ';' if paren == 0 => { flush(&mut text, &mut toks); toks.push(CssTok::Semi); }
            c if c.is_whitespace() => { flush(&mut text, &mut toks); if toks.last() != Some(&CssTok::Space) { toks.push(CssTok::Space); } }
            _ => text.push(c),
        }
        i += 1;
    }
    flush(&mut text, &mut toks);
    Ok(toks)
}

fn css_beautify(src: &str, scss: bool) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut depth = 0usize;
    let emit = |out: &mut Vec<String>, depth: usize, s: &str| out.push(format!("{}{}", "  ".repeat(depth), s));
    for tok in css_tokens(src, scss)? {
        match tok {
            CssTok::Text(s) => line.push_str(&s),
            CssTok::Space => if !line.is_empty() && !line.ends_with(' ') { line.push(' ') },
            CssTok::Comment(c) => {
                if !line.trim().is_empty() { emit(&mut out, depth, line.trim()); }
                line.clear();
                emit(&mut out, depth, &c);
            }
            CssTok::Open => { emit(&mut out, depth, &format!("{} {{", line.trim()).trim_start().to_string()); line.clear(); depth += 1; }
            CssTok::Semi => { if !line.trim().is_empty() { emit(&mut out, depth, &format!("{};", line.trim())); } line.clear(); }
            CssTok::Close => {
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
    while out.last().is_some_and(|l| l.is_empty()) { out.pop(); }
    Ok(out.join("\n"))
}

fn css_minify(src: &str, scss: bool) -> Result<String, String> {
    let mut out = String::new();
    let mut space = false;
    for tok in css_tokens(src, scss)? {
        match tok {
            CssTok::Space | CssTok::Comment(_) => space = true,
            CssTok::Text(s) => {
                if space && !out.is_empty() && !out.ends_with(['{', '}', ';', ':', ',']) { out.push(' '); }
                out.push_str(&s);
                space = false;
            }
            CssTok::Open => { out.push('{'); space = false; }
            CssTok::Semi => { if !out.ends_with(['{', ';']) { out.push(';'); } space = false; }
            CssTok::Close => { if out.ends_with(';') { out.pop(); } out.push('}'); space = false; }
        }
    }
    Ok(out)
}

// ---------- HTML ----------

#[derive(Debug)]
enum HtmlTok<'a> { Open { name: String, raw: &'a str, closed: bool }, Close { name: String, raw: &'a str }, Text(&'a str), Other(&'a str), Raw(&'a str) }

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
const RAW: &[&str] = &["script", "style", "pre", "textarea"];

fn tag_name(raw: &str) -> String {
    raw.trim_start_matches(['<', '/']).chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':').collect::<String>().to_ascii_lowercase()
}

fn html_tokens(src: &str) -> Result<Vec<HtmlTok<'_>>, String> {
    let bytes = src.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &src[i..];
        let end = if rest.starts_with("<!--") {
            let end = i + rest.find("-->").ok_or("Unterminated comment")? + 3;
            toks.push(HtmlTok::Other(&src[i..end]));
            end
        } else if rest.starts_with("<!") || rest.starts_with("<?") {
            let end = i + rest.find('>').ok_or("Unterminated declaration")? + 1;
            toks.push(HtmlTok::Other(&src[i..end]));
            end
        } else if rest.starts_with("</") {
            let end = i + rest.find('>').ok_or("Unterminated closing tag")? + 1;
            toks.push(HtmlTok::Close { name: tag_name(&src[i..end]), raw: &src[i..end] });
            end
        } else if rest.starts_with('<') && bytes.get(i + 1).is_some_and(|b| b.is_ascii_alphabetic()) {
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
            let closed = raw.ends_with("/>");
            let mut end = j + 1;
            if RAW.contains(&name.as_str()) && !closed {
                let close = format!("</{name}");
                let body_end = src[end..].to_ascii_lowercase().find(&close).map_or(src.len(), |k| end + k);
                toks.push(HtmlTok::Open { name, raw, closed });
                if body_end > end { toks.push(HtmlTok::Raw(&src[end..body_end])); }
                end = body_end;
            } else {
                toks.push(HtmlTok::Open { name, raw, closed });
            }
            end
        } else {
            let next = rest[1..].find('<').map_or(src.len(), |k| i + 1 + k);
            toks.push(HtmlTok::Text(&src[i..next]));
            next
        };
        i = end;
    }
    Ok(toks)
}

fn collapse(text: &str) -> String { text.split_whitespace().collect::<Vec<_>>().join(" ") }

/// Puts each element on its own line. An element whose only content is text stays on one line;
/// `pre` and `textarea` keep their content untouched, `script` and `style` are re-indented.
fn html_beautify(src: &str) -> Result<String, String> {
    let toks = html_tokens(src)?;
    let mut out: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let pad = |depth: usize| "  ".repeat(depth);
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            HtmlTok::Open { name, raw, closed } => {
                if *closed || VOID.contains(&name.as_str()) { out.push(format!("{}{raw}", pad(depth))); }
                else if let (Some(HtmlTok::Raw(body)), true) = (toks.get(i + 1), name == "pre" || name == "textarea") {
                    let close = match toks.get(i + 2) { Some(HtmlTok::Close { raw: c, .. }) => { i += 1; *c } _ => "" };
                    out.push(format!("{}{raw}{body}{close}", pad(depth)));
                    i += 1;
                } else if let Some(HtmlTok::Raw(body)) = toks.get(i + 1) {
                    out.push(format!("{}{raw}", pad(depth)));
                    let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
                    let common = lines.iter().map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
                    for line in lines { out.push(format!("{}{}", pad(depth + 1), &line[common..].trim_end())); }
                    if let Some(HtmlTok::Close { raw: c, .. }) = toks.get(i + 2) { out.push(format!("{}{c}", pad(depth))); i += 1; }
                    i += 1;
                } else if let (Some(HtmlTok::Text(t)), Some(HtmlTok::Close { name: n, raw: c })) = (toks.get(i + 1), toks.get(i + 2)) {
                    if n == name { out.push(format!("{}{raw}{}{c}", pad(depth), collapse(t))); i += 2; }
                    else { out.push(format!("{}{raw}", pad(depth))); depth += 1; }
                } else if let Some(HtmlTok::Close { name: n, raw: c }) = toks.get(i + 1) {
                    if n == name { out.push(format!("{}{raw}{c}", pad(depth))); i += 1; }
                    else { out.push(format!("{}{raw}", pad(depth))); depth += 1; }
                } else {
                    out.push(format!("{}{raw}", pad(depth)));
                    depth += 1;
                }
            }
            HtmlTok::Close { raw, .. } => { depth = depth.saturating_sub(1); out.push(format!("{}{raw}", pad(depth))); }
            HtmlTok::Text(t) => { let t = collapse(t); if !t.is_empty() { out.push(format!("{}{t}", pad(depth))); } }
            HtmlTok::Other(o) => out.push(format!("{}{}", pad(depth), o.trim())),
            HtmlTok::Raw(body) => out.push(body.to_string()),
        }
        i += 1;
    }
    Ok(out.join("\n"))
}

fn html_minify(src: &str) -> Result<String, String> {
    let mut cfg = minify_html::Cfg::new();
    cfg.minify_css = true;
    cfg.minify_js = true;
    cfg.keep_closing_tags = true;
    cfg.keep_html_and_head_opening_tags = true;
    String::from_utf8(minify_html::minify(src.as_bytes(), &cfg)).map_err(|e| e.to_string())
}

// ---------- JavaScript ----------

fn js_rewrite(src: &str, minify: bool) -> Result<String, String> {
    use oxc::{allocator::Allocator, codegen::{Codegen, CodegenOptions}, minifier::{Minifier, MinifierOptions}, parser::Parser, span::SourceType};
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, src, SourceType::mjs().with_jsx(true)).parse();
    if let Some(error) = parsed.diagnostics.first() {
        let line = error.labels.as_ref().first().map(|l| src[..(l.offset() as usize).min(src.len())].lines().count().max(1));
        return Err(match line { Some(n) => format!("Line {n}: {error}"), None => error.to_string() });
    }
    let mut program = parsed.program;
    if minify {
        let result = Minifier::new(MinifierOptions::default()).minify(&allocator, &mut program);
        Ok(Codegen::new().with_options(CodegenOptions::minify()).with_scoping(result.scoping)
            .with_private_member_mappings(result.class_private_mappings).build(&program).code)
    } else {
        let options = CodegenOptions { indent_char: oxc::codegen::IndentChar::Space, indent_width: 2, ..CodegenOptions::default() };
        Ok(Codegen::new().with_options(options).with_source_text(src).build(&program).code)
    }
}

// ---------- XML ----------

fn xml_rewrite(src: &str, indent: bool) -> Result<String, String> {
    use quick_xml::{events::Event, Reader, Writer};
    let mut reader = Reader::from_str(src);
    reader.config_mut().trim_text(true);
    let mut writer = if indent { Writer::new_with_indent(Vec::new(), b' ', 2) } else { Writer::new(Vec::new()) };
    loop {
        let event = reader.read_event().map_err(|e| {
            let pos = (reader.error_position() as usize).min(src.len());
            format!("Line {}: {e}", src[..pos].lines().count().max(1))
        })?;
        match event {
            Event::Eof => break,
            event => writer.write_event(event).map_err(|e| e.to_string())?,
        }
    }
    String::from_utf8(writer.into_inner()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_beautify_and_minify() {
        let src = "a,b{color:red;margin:0 auto}/* note */\n@media (max-width: 10px){.x{background:url(data:a;b)}}";
        assert_eq!(css_beautify(src, false).unwrap(),
            "a,b {\n  color:red;\n  margin:0 auto;\n}\n\n/* note */\n@media (max-width: 10px) {\n  .x {\n    background:url(data:a;b);\n  }\n}");
        assert_eq!(css_minify(&css_beautify(src, false).unwrap(), false).unwrap(),
            "a,b{color:red;margin:0 auto}@media (max-width:10px){.x{background:url(data:a;b)}}");
        assert_eq!(css_minify("a { content: \"x ; }\" ; }", false).unwrap(), "a{content:\"x ; }\"}");
        assert!(css_beautify("a {", false).is_err());
        assert!(css_beautify("}", false).is_err());
    }

    #[test]
    fn scss_comments_nesting_and_interpolation() {
        let src = "// top\n$w: 10px;\n.a { &:hover { width: #{$w}; } // inline\n}";
        assert_eq!(css_beautify(src, true).unwrap(),
            "// top\n$w: 10px;\n.a {\n  &:hover {\n    width: #{$w};\n  }\n  // inline\n}");
        assert_eq!(css_minify(src, true).unwrap(), "$w:10px;.a{&:hover{width:#{$w}}}");
        // In plain CSS `//` is not a comment (e.g. inside url()).
        assert_eq!(css_minify("a { b: url(http://x) }", false).unwrap(), "a{b:url(http://x)}");
    }

    #[test]
    fn html_beautify_layout() {
        let src = "<!DOCTYPE html><html><head><title>Hi</title><style>\n    a { b: c }\n</style></head><body><p>Hello   world</p><br><div><span>x</span></div><pre>  keep\n   this</pre><script>let a = '<div>';</script></body></html>";
        assert_eq!(html_beautify(src).unwrap(), [
            "<!DOCTYPE html>", "<html>", "  <head>", "    <title>Hi</title>", "    <style>", "      a { b: c }", "    </style>", "  </head>",
            "  <body>", "    <p>Hello world</p>", "    <br>", "    <div>", "      <span>x</span>", "    </div>",
            "    <pre>  keep\n   this</pre>", "    <script>", "      let a = '<div>';", "    </script>", "  </body>", "</html>",
        ].join("\n"));
        assert!(html_beautify("<a href=\"x").is_err());
    }

    #[test]
    fn html_minify_shrinks() {
        let out = html_minify("<div>\n   <p>  Hello  </p>\n</div>").unwrap();
        assert!(out.len() < 25, "{out}");
        assert!(out.contains("Hello"));
    }

    #[test]
    fn js_rewrite_round_trip() {
        let pretty = js_rewrite("function add(a,b){return a+b}\nconst x=add(1,2)", false).unwrap();
        assert!(pretty.contains("function add(a, b) {\n  return a + b;\n}"), "{pretty}");
        let min = js_rewrite("function add(first, second) { return first + second; }\nexport const total = add(1, 2);", true).unwrap();
        assert!(min.len() < 60, "{min}");
        let err = js_rewrite("let = ;\nfoo(", false).unwrap_err();
        assert!(err.starts_with("Line 1"), "{err}");
    }

    #[test]
    fn xml_rewrite_round_trip() {
        let pretty = xml_rewrite("<a><b x=\"1\">t</b><c/></a>", true).unwrap();
        assert_eq!(pretty, "<a>\n  <b x=\"1\">t</b>\n  <c/>\n</a>");
        assert_eq!(xml_rewrite(&pretty, false).unwrap(), "<a><b x=\"1\">t</b><c/></a>");
        assert!(xml_rewrite("<a><b></a>", true).is_err());
    }

    #[test]
    fn sql_formats() {
        let tool_out = sql_format("select a,b from t where x=1").unwrap();
        assert!(tool_out.starts_with("SELECT\n"), "{tool_out}");
        assert!(tool_out.contains("WHERE"));
    }
}
