//! Beautify / minify tools for code and markup.
//!
//! JavaScript, XML, SQL and HTML minification use established crates (oxc,
//! quick-xml, sqlformat, minify-html). CSS/SCSS and HTML beautification are
//! implemented in `css.rs` and `html.rs`: no crate keeps comments and supports
//! SCSS, and none pretty-prints HTML.
use super::{Category, TextTool, Tool};

mod css;
mod html;

pub fn html() -> Box<dyn Tool> {
    Box::new(TextTool::new("HTML Beautify/Minify", "HT", Category::Code, vec![("Beautify", html::beautify), ("Minify", html::minify)]))
}

pub fn css() -> Box<dyn Tool> {
    Box::new(TextTool::new("CSS Beautify/Minify", "CS", Category::Code, vec![("Beautify", |s| css::beautify(s, false)), ("Minify", |s| css::minify(s, false))]))
}

pub fn scss() -> Box<dyn Tool> {
    Box::new(TextTool::new("SCSS Beautify/Minify", "SC", Category::Code, vec![("Beautify", |s| css::beautify(s, true)), ("Minify", |s| css::minify(s, true))]))
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
