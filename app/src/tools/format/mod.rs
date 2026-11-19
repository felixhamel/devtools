//! Beautify / minify tools for code and markup.
//!
//! JavaScript, XML, SQL and HTML minification use established crates (oxc,
//! quick-xml, sqlformat, minify-html). CSS/SCSS and HTML beautification are
//! implemented in `css.rs` and `html.rs`: no crate keeps comments and supports
//! SCSS, and none pretty-prints HTML.
use super::{Category, TextTool, Tool};
use crate::ui::icon;

mod css;
mod html;

pub fn html() -> Box<dyn Tool> {
    Box::new(TextTool::new("HTML Beautify/Minify", icon::FILE_HTML, Category::Code, vec![("Beautify", html::beautify), ("Minify", html::minify)])
        .describe("Indents HTML one element per line (inline <style> and <script> included), or minifies it.").sample(r#"<!DOCTYPE html><html><head><title>Hello</title><style>body{font-family:system-ui}</style></head><body><nav><a href="/">Home</a><a href="/about">About</a></nav><main><h1>Hello, world</h1><p>Some <b>bold</b> text.</p></main></body></html>"#).syntax("html"))
}

pub fn css() -> Box<dyn Tool> {
    Box::new(TextTool::new("CSS/SCSS Beautify/Minify", icon::FILE_CSS, Category::Code, vec![("Beautify", css::beautify), ("Minify", css::minify)])
        .describe("Formats CSS and SCSS (nesting, // comments and #{…} interpolation are understood).")
        .sample(r#"$accent:#56d28f;
// card component
.card{padding:8px;&:hover{color:$accent}.title{font-weight:bold;margin:#{$gap} 0}}
@media (max-width:600px){.nav{display:none}}"#).syntax("css"))
}

pub fn js() -> Box<dyn Tool> {
    Box::new(TextTool::new("JS Beautify/Minify", icon::FILE_JS, Category::Code, vec![("Beautify", |s| js_rewrite(s, false)), ("Minify", |s| js_rewrite(s, true))])
        .describe("Reprints JavaScript (JSX allowed). Beautify normalizes quotes and spacing; Minify also shortens names.").sample(r#"// greet someone
function greet(name){const message=`Hello, ${name}!`;return message}
export const users=['Ada','Grace'].map(greet);console.log(users)"#).syntax("js"))
}

pub fn xml() -> Box<dyn Tool> {
    Box::new(TextTool::new("XML Beautify/Minify", icon::FILE_CODE, Category::Code, vec![("Beautify", |s| xml_rewrite(s, true)), ("Minify", |s| xml_rewrite(s, false))])
        .describe("Indents XML (well-formedness is checked, comments and CDATA are kept), or minifies it.").sample(r#"<?xml version="1.0"?><note id="42"><to>Ada</to><from>Grace</from><body>Hello <b>there</b></body><tags/></note>"#).syntax("xml"))
}

pub fn sql() -> Box<dyn Tool> {
    Box::new(TextTool::new("SQL Formatter", icon::DATABASE, Category::Code, vec![("Format", sql_format)])
        .describe("Formats SQL with one clause per line and uppercase keywords. Most dialects are understood.").sample(r#"select u.id, u.name, count(o.id) as orders from users u left join orders o on o.user_id = u.id where u.active = true group by u.id, u.name order by orders desc limit 10"#).syntax("sql"))
}

fn sql_format(src: &str) -> Result<String, String> {
    Ok(sqlformat::format(src, &sqlformat::QueryParams::None, &sqlformat::FormatOptions { uppercase: Some(true), ..Default::default() }))
}

/// 1-based line number of a byte offset, for error messages.
fn line_of(src: &str, offset: usize) -> usize {
    src[..offset.min(src.len())].lines().count().max(1)
}

/// Parses JavaScript with oxc and prints it back, either pretty (2-space indent,
/// comments kept) or minified (compressed and with shortened names).
fn js_rewrite(src: &str, minify: bool) -> Result<String, String> {
    use oxc::{allocator::Allocator, codegen::{Codegen, CodegenOptions, IndentChar}, minifier::{Minifier, MinifierOptions}, parser::Parser, span::SourceType};
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, src, SourceType::mjs().with_jsx(true)).parse();
    if let Some(error) = parsed.diagnostics.first() {
        let line = error.labels.as_ref().first().map(|label| line_of(src, label.offset() as usize));
        return Err(match line { Some(n) => format!("Line {n}: {error}"), None => error.to_string() });
    }
    let mut program = parsed.program;
    if minify {
        let result = Minifier::new(MinifierOptions::default()).minify(&allocator, &mut program);
        Ok(Codegen::new().with_options(CodegenOptions::minify()).with_scoping(result.scoping)
            .with_private_member_mappings(result.class_private_mappings).build(&program).code)
    } else {
        let options = CodegenOptions { indent_char: IndentChar::Space, indent_width: 2, ..CodegenOptions::default() };
        // The source text is needed for codegen to carry comments over.
        Ok(Codegen::new().with_options(options).with_source_text(src).build(&program).code)
    }
}

/// Re-writes XML event by event: indented with 2 spaces, or with all
/// whitespace-only text removed. Mismatched tags are reported with their line.
fn xml_rewrite(src: &str, indent: bool) -> Result<String, String> {
    use quick_xml::{events::Event, Reader, Writer};
    let mut reader = Reader::from_str(src);
    reader.config_mut().trim_text(true);
    let mut writer = if indent { Writer::new_with_indent(Vec::new(), b' ', 2) } else { Writer::new(Vec::new()) };
    loop {
        let event = reader.read_event()
            .map_err(|e| format!("Line {}: {e}", line_of(src, reader.error_position() as usize)))?;
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
        let out = sql_format("select a,b from t where x=1").unwrap();
        assert!(out.starts_with("SELECT\n"), "{out}");
        assert!(out.contains("WHERE"));
    }
}
