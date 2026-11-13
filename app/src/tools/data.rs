use serde_json::Value;
use super::{Category, TextTool, Tool};

pub fn json_format() -> Box<dyn Tool> {
    Box::new(TextTool::new("JSON Format/Validate", "{}", Category::Data, vec![("Format", json_pretty), ("Minify", json_minify)]))
}

fn json_pretty(input: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

fn json_minify(input: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

pub fn url_parser() -> Box<dyn Tool> {
    Box::new(TextTool::new("URL Parser", "URL", Category::Data, vec![("Parse", parse_url)])
        .describe("Paste a URL to split it into its parts and decoded query parameters."))
}

fn parse_url(input: &str) -> Result<String, String> {
    let url = url::Url::parse(input.trim()).map_err(|e| format!("Invalid URL: {e}"))?;
    let or_none = |s: Option<&str>| s.filter(|s| !s.is_empty()).unwrap_or("(none)").to_string();
    let mut out = format!(
        "Scheme: {}\nUsername: {}\nPassword: {}\nHost: {}\nPort: {}\nPath: {}\nQuery: {}\nFragment: {}",
        url.scheme(), or_none(Some(url.username())), or_none(url.password()), or_none(url.host_str()),
        url.port_or_known_default().map_or("(none)".into(), |p| p.to_string()), url.path(), or_none(url.query()), or_none(url.fragment()),
    );
    let params: Vec<_> = url.query_pairs().collect();
    if !params.is_empty() {
        out.push_str("\n\nQuery parameters:");
        for (key, value) in params { out.push_str(&format!("\n  {key} = {value}")); }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_format_keeps_key_order() {
        assert_eq!(json_minify("{\"b\": 1, \"a\": {\"z\": 2, \"y\": 3}}").unwrap(), "{\"b\":1,\"a\":{\"z\":2,\"y\":3}}");
    }

    #[test]
    fn parses_url() {
        let out = parse_url("https://user:pw@example.com/a/b?x=1&y=hello%20world#top").unwrap();
        assert!(out.contains("Host: example.com"));
        assert!(out.contains("Port: 443"));
        assert!(out.contains("Password: pw"));
        assert!(out.contains("y = hello world"));
        assert!(out.contains("Fragment: top"));
        assert!(parse_url("not a url").is_err());
    }
}
