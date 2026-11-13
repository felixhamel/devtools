//! Turns a cURL command into equivalent JavaScript, Python or Rust code.
use base64::{engine::general_purpose::STANDARD, Engine};
use super::{Category, TextTool, Tool};

pub fn curl_to_code() -> Box<dyn Tool> {
    Box::new(TextTool::new("cURL to Code", "cR", Category::Code, vec![
        ("JavaScript (fetch)", |s| parse_curl(s).map(|r| to_js(&r))),
        ("Python (requests)", |s| parse_curl(s).map(|r| to_python(&r))),
        ("Rust (reqwest)", |s| parse_curl(s).map(|r| to_rust(&r))),
    ]).describe("Paste a curl command (multi-line with \\ is fine). Supports -X, -H, -d/--data*, --json, -u, -A, -b, -e, -G, -I."))
}

#[derive(Debug, Default, PartialEq)]
struct Request { method: String, url: String, headers: Vec<(String, String)>, body: Option<String> }

/// Splits the command like a shell would, then interprets the curl options.
/// The method defaults to GET, or POST when a body is given.
fn parse_curl(src: &str) -> Result<Request, String> {
    let joined = src.replace("\\\r\n", " ").replace("\\\n", " ");
    let args = shlex::split(&joined).ok_or("Unbalanced quotes in the command")?;
    let mut args = args.into_iter().peekable();
    if args.peek().is_some_and(|a| a == "curl") { args.next(); }
    let mut req = Request::default();
    let mut data: Vec<String> = Vec::new();
    let (mut get, mut head) = (false, false);
    // Options whose value we do not use, so it must be skipped.
    const IGNORED_WITH_VALUE: &[&str] = &["-o", "--output", "-w", "--write-out", "-m", "--max-time", "--connect-timeout", "--retry", "-x", "--proxy", "--cacert", "--cert", "--key", "-c", "--cookie-jar", "-r", "--range"];
    while let Some(arg) = args.next() {
        // Short options may carry their value attached, e.g. -XPOST or -H'Accept: x'.
        let (flag, attached) = match arg.get(..2) {
            Some(short @ ("-X" | "-H" | "-d" | "-u" | "-A" | "-b" | "-e")) if arg.len() > 2 && !arg.starts_with("--") => (short.to_string(), Some(arg[2..].to_string())),
            _ => (arg.clone(), None),
        };
        let mut value = |name: &str| attached.clone().or_else(|| args.next()).ok_or(format!("{name} needs a value"));
        match flag.as_str() {
            "-X" | "--request" => req.method = value(&flag)?.to_uppercase(),
            "-H" | "--header" => {
                let h = value(&flag)?;
                let (k, v) = h.split_once(':').ok_or(format!("Invalid header: {h}"))?;
                req.headers.push((k.trim().to_string(), v.trim().to_string()));
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" => {
                let d = value(&flag)?;
                if d.starts_with('@') && flag != "--data-raw" { return Err(format!("Reading the body from a file ({d}) is not supported")); }
                data.push(d);
            }
            "--data-urlencode" => {
                let d = value(&flag)?;
                data.push(match d.split_once('=') { Some((k, v)) => format!("{k}={}", urlencoding::encode(v)), None => urlencoding::encode(&d).into_owned() });
            }
            "--json" => {
                data.push(value(&flag)?);
                req.headers.push(("Content-Type".into(), "application/json".into()));
                req.headers.push(("Accept".into(), "application/json".into()));
            }
            "-u" | "--user" => req.headers.push(("Authorization".into(), format!("Basic {}", STANDARD.encode(value(&flag)?)))),
            "-A" | "--user-agent" => req.headers.push(("User-Agent".into(), value(&flag)?)),
            "-b" | "--cookie" => req.headers.push(("Cookie".into(), value(&flag)?)),
            "-e" | "--referer" => req.headers.push(("Referer".into(), value(&flag)?)),
            "--url" => req.url = value(&flag)?,
            "-G" | "--get" => get = true,
            "-I" | "--head" => head = true,
            "-F" | "--form" => return Err("Multipart forms (-F) are not supported".into()),
            f if IGNORED_WITH_VALUE.contains(&f) => { value(f)?; }
            f if f.starts_with('-') => {} // flags such as -L, -k, -s, --compressed
            _ => req.url = arg,
        }
    }
    if req.url.is_empty() { return Err("No URL found in the command".into()); }
    if !data.is_empty() {
        let body = data.join("&");
        if get { req.url = format!("{}{}{body}", req.url, if req.url.contains('?') { '&' } else { '?' }); }
        else {
            req.body = Some(body);
            if !req.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-type")) {
                req.headers.push(("Content-Type".into(), "application/x-www-form-urlencoded".into()));
            }
        }
    }
    if req.method.is_empty() { req.method = if head { "HEAD" } else if req.body.is_some() { "POST" } else { "GET" }.into(); }
    Ok(req)
}

/// A double-quoted string literal valid in both JavaScript and Python.
fn quote(s: &str) -> String { serde_json::to_string(s).unwrap() }

fn to_js(r: &Request) -> String {
    let mut out = format!("const response = await fetch({}, {{\n  method: {},\n", quote(&r.url), quote(&r.method));
    if !r.headers.is_empty() {
        out.push_str("  headers: {\n");
        for (k, v) in &r.headers { out.push_str(&format!("    {}: {},\n", quote(k), quote(v))); }
        out.push_str("  },\n");
    }
    if let Some(body) = &r.body { out.push_str(&format!("  body: {},\n", quote(body))); }
    out.push_str("});\n\nconst text = await response.text();\nconsole.log(response.status, text);");
    out
}

fn to_python(r: &Request) -> String {
    let mut out = String::from("import requests\n\n");
    if !r.headers.is_empty() {
        out.push_str("headers = {\n");
        for (k, v) in &r.headers { out.push_str(&format!("    {}: {},\n", quote(k), quote(v))); }
        out.push_str("}\n");
    }
    if let Some(body) = &r.body { out.push_str(&format!("data = {}\n", quote(body))); }
    out.push_str(&format!("\nresponse = requests.request({}, {}", quote(&r.method), quote(&r.url)));
    if !r.headers.is_empty() { out.push_str(", headers=headers"); }
    if r.body.is_some() { out.push_str(", data=data"); }
    out.push_str(")\nprint(response.status_code, response.text)");
    out
}

/// Uses `{:?}` for literals: Rust's debug formatting always yields a valid, escaped string literal.
fn to_rust(r: &Request) -> String {
    let method = match r.method.as_str() {
        m @ ("GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") => format!(".{}({:?})", m.to_lowercase(), r.url),
        m => format!(".request(reqwest::Method::from_bytes(b{m:?})?, {:?})", r.url),
    };
    let mut out = format!("// Cargo.toml: reqwest = {{ version = \"0.12\", features = [\"blocking\"] }}\nfn main() -> Result<(), Box<dyn std::error::Error>> {{\n    let client = reqwest::blocking::Client::new();\n    let response = client\n        {method}\n");
    for (k, v) in &r.headers { out.push_str(&format!("        .header({k:?}, {v:?})\n")); }
    if let Some(body) = &r.body { out.push_str(&format!("        .body({body:?})\n")); }
    out.push_str("        .send()?;\n    println!(\"{} {}\", response.status(), response.text()?);\n    Ok(())\n}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_flags() {
        let r = parse_curl("curl -XPOST 'https://api.test/items?x=1' \\\n  -H 'Content-Type: application/json' -H\"X-Token: a b\" \\\n  -d '{\"name\":\"it'\\''s\"}' -L --compressed -o out.txt").unwrap();
        assert_eq!(r.method, "POST");
        assert_eq!(r.url, "https://api.test/items?x=1");
        assert_eq!(r.headers, vec![("Content-Type".into(), "application/json".into()), ("X-Token".into(), "a b".into())]);
        assert_eq!(r.body.as_deref(), Some("{\"name\":\"it's\"}"));

        let r = parse_curl("curl -G https://x.test/s -d q=rust -d page=2 -u ada:pw").unwrap();
        assert_eq!((r.method.as_str(), r.url.as_str(), r.body), ("GET", "https://x.test/s?q=rust&page=2", None));
        assert_eq!(r.headers, vec![("Authorization".into(), "Basic YWRhOnB3".into())]);

        let r = parse_curl("curl https://x.test -d a=1").unwrap();
        assert_eq!(r.method, "POST");
        assert!(r.headers.contains(&("Content-Type".into(), "application/x-www-form-urlencoded".into())));

        assert!(parse_curl("curl -H 'X: 1'").is_err());
        assert!(parse_curl("curl 'https://x").is_err());
        assert!(parse_curl("curl https://x -F a=@f").is_err());
    }

    #[test]
    fn generates_code() {
        let r = parse_curl("curl -X PUT https://api.test/a -H 'Accept: text/plain' -d 'x=\"1\"'").unwrap();
        let js = to_js(&r);
        assert!(js.contains("fetch(\"https://api.test/a\", {\n  method: \"PUT\",\n  headers: {\n    \"Accept\": \"text/plain\","), "{js}");
        assert!(js.contains("body: \"x=\\\"1\\\"\","), "{js}");
        let py = to_python(&r);
        assert!(py.contains("response = requests.request(\"PUT\", \"https://api.test/a\", headers=headers, data=data)"), "{py}");
        let rs = to_rust(&r);
        assert!(rs.contains("        .put(\"https://api.test/a\")\n        .header(\"Accept\", \"text/plain\")"), "{rs}");
        let rs = to_rust(&parse_curl("curl -X PURGE https://c.test").unwrap());
        assert!(rs.contains(".request(reqwest::Method::from_bytes(b\"PURGE\")?, \"https://c.test\")"), "{rs}");
    }
}
