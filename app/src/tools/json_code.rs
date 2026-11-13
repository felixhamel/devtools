//! Infers types from a JSON sample and prints them as TypeScript, Rust (serde) or Go.
use serde_json::Value;
use super::{Category, TextTool, Tool};

pub fn json_to_code() -> Box<dyn Tool> {
    Box::new(TextTool::new("JSON to Code", "{C}", Category::Data, vec![
        ("TypeScript", |s| generate(s, Lang::TypeScript)),
        ("Rust (serde)", |s| generate(s, Lang::Rust)),
        ("Go", |s| generate(s, Lang::Go)),
    ]).describe("Paste a JSON sample. Arrays of objects are merged: keys missing from some items become optional."))
}

#[derive(Clone, Copy, PartialEq)]
enum Lang { TypeScript, Rust, Go }

#[derive(Clone, PartialEq, Debug)]
enum Ty { Unknown, Bool, Int, Float, Str, Array(Box<Ty>), Obj(Vec<Field>), Any }

#[derive(Clone, PartialEq, Debug)]
struct Field { key: String, ty: Ty, optional: bool }

fn infer(value: &Value) -> Ty {
    match value {
        Value::Null => Ty::Unknown,
        Value::Bool(_) => Ty::Bool,
        Value::Number(n) => if n.is_f64() { Ty::Float } else { Ty::Int },
        Value::String(_) => Ty::Str,
        Value::Array(items) => Ty::Array(Box::new(items.iter().map(infer).fold(Ty::Unknown, merge))),
        Value::Object(map) => Ty::Obj(map.iter().map(|(k, v)| Field { key: k.clone(), ty: infer(v), optional: v.is_null() }).collect()),
    }
}

fn merge(a: Ty, b: Ty) -> Ty {
    match (a, b) {
        (Ty::Unknown, t) | (t, Ty::Unknown) => t,
        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => Ty::Float,
        (Ty::Array(x), Ty::Array(y)) => Ty::Array(Box::new(merge(*x, *y))),
        (Ty::Obj(a), Ty::Obj(b)) => {
            let mut fields: Vec<Field> = a.into_iter().map(|mut f| { if !b.iter().any(|g| g.key == f.key) { f.optional = true; } f }).collect();
            for g in b {
                match fields.iter_mut().find(|f| f.key == g.key) {
                    Some(f) => { f.ty = merge(f.ty.clone(), g.ty); f.optional |= g.optional; }
                    None => fields.push(Field { optional: true, ..g }),
                }
            }
            Ty::Obj(fields)
        }
        (a, b) if a == b => a,
        _ => Ty::Any,
    }
}

/// Splits `firstName`, `first_name` or `first-name` into lowercase words.
fn words(key: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;
    for c in key.chars() {
        if !c.is_alphanumeric() { if !current.is_empty() { words.push(std::mem::take(&mut current)); } prev_lower = false; continue; }
        if c.is_uppercase() && prev_lower && !current.is_empty() { words.push(std::mem::take(&mut current)); }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() { words.push(current); }
    words
}

fn pascal(key: &str) -> String {
    let name: String = words(key).iter().map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }).collect();
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) { format!("Field{name}") } else { name }
}

fn snake(key: &str) -> String {
    const KEYWORDS: &[&str] = &["as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while"];
    let name = words(key).join("_");
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) { format!("field_{name}") }
    else if KEYWORDS.contains(&name.as_str()) { format!("{name}_") } else { name }
}

struct Gen { lang: Lang, structs: Vec<(String, Vec<Field>)>, used: Vec<String> }

impl Gen {
    fn unique(&mut self, hint: &str) -> String {
        let base = pascal(hint);
        let mut name = base.clone();
        let mut n = 2;
        while self.used.contains(&name) { name = format!("{base}{n}"); n += 1; }
        self.used.push(name.clone());
        name
    }

    fn expr(&mut self, ty: &Ty, hint: &str) -> String {
        let l = self.lang;
        let pick = |ts: &str, rs: &str, go: &str| match l { Lang::TypeScript => ts, Lang::Rust => rs, Lang::Go => go }.to_string();
        match ty {
            Ty::Unknown | Ty::Any => pick("unknown", "serde_json::Value", "any"),
            Ty::Bool => pick("boolean", "bool", "bool"),
            Ty::Int => pick("number", "i64", "int64"),
            Ty::Float => pick("number", "f64", "float64"),
            Ty::Str => pick("string", "String", "string"),
            Ty::Array(inner) => {
                let inner = self.expr(inner, &format!("{hint} item"));
                match l { Lang::TypeScript => format!("{inner}[]"), Lang::Rust => format!("Vec<{inner}>"), Lang::Go => format!("[]{inner}") }
            }
            Ty::Obj(fields) => {
                let name = self.unique(hint);
                self.structs.push((name.clone(), fields.clone()));
                name
            }
        }
    }

    fn render_struct(&mut self, name: &str, fields: &[Field]) -> String {
        let mut lines = Vec::new();
        match self.lang {
            Lang::TypeScript => lines.push(format!("export interface {name} {{")),
            Lang::Rust => { lines.push("#[derive(Debug, Clone, Serialize, Deserialize)]".into()); lines.push(format!("pub struct {name} {{")); }
            Lang::Go => lines.push(format!("type {name} struct {{")),
        }
        for f in fields {
            let ty = self.expr(&f.ty, &f.key);
            match self.lang {
                Lang::TypeScript => {
                    let simple = !f.key.is_empty() && f.key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') && !f.key.starts_with(|c: char| c.is_ascii_digit());
                    let key = if simple { f.key.clone() } else { serde_json::to_string(&f.key).unwrap() };
                    lines.push(if f.optional { format!("  {key}?: {ty} | null;") } else { format!("  {key}: {ty};") });
                }
                Lang::Rust => {
                    let field = snake(&f.key);
                    if field != f.key { lines.push(format!("    #[serde(rename = {:?})]", f.key)); }
                    lines.push(if f.optional { format!("    pub {field}: Option<{ty}>,") } else { format!("    pub {field}: {ty},") });
                }
                Lang::Go => {
                    let (ty, omit) = if f.optional { (format!("*{ty}"), ",omitempty") } else { (ty, "") };
                    lines.push(format!("\t{} {ty} `json:\"{}{omit}\"`", pascal(&f.key), f.key));
                }
            }
        }
        lines.push("}".into());
        lines.join("\n")
    }
}

fn generate(src: &str, lang: Lang) -> Result<String, String> {
    let value: Value = serde_json::from_str(src).map_err(|e| e.to_string())?;
    let mut generator = Gen { lang, structs: Vec::new(), used: Vec::new() };
    let root = infer(&value);
    let root_expr = generator.expr(&root, "root");
    let mut blocks = Vec::new();
    if !matches!(root, Ty::Obj(_)) {
        blocks.push(match lang {
            Lang::TypeScript => format!("export type Root = {root_expr};"),
            Lang::Rust => format!("pub type Root = {root_expr};"),
            Lang::Go => format!("type Root {root_expr}"),
        });
    }
    // Rendering a struct can register nested ones, so walk the list as it grows.
    let mut i = 0;
    while i < generator.structs.len() {
        let (name, fields) = generator.structs[i].clone();
        let block = generator.render_struct(&name, &fields);
        blocks.push(block);
        i += 1;
    }
    let header = match lang { Lang::Rust => "use serde::{Deserialize, Serialize};\n\n", Lang::Go => "package main\n\n", Lang::TypeScript => "" };
    Ok(format!("{header}{}", blocks.join("\n\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = r#"{"id": 1, "firstName": "Ada", "score": 9.5, "tags": ["a"], "type": "x",
        "items": [{"sku": "A", "qty": 1}, {"sku": "B", "qty": 2.5, "note": null}]}"#;

    #[test]
    fn typescript() {
        let out = generate(SAMPLE, Lang::TypeScript).unwrap();
        assert!(out.contains("export interface Root {\n  id: number;\n  firstName: string;\n  score: number;\n  tags: string[];\n  type: string;\n  items: ItemsItem[];\n}"), "{out}");
        assert!(out.contains("export interface ItemsItem {\n  sku: string;\n  qty: number;\n  note?: unknown | null;\n}"), "{out}");
    }

    #[test]
    fn rust() {
        let out = generate(SAMPLE, Lang::Rust).unwrap();
        assert!(out.contains("    #[serde(rename = \"firstName\")]\n    pub first_name: String,"), "{out}");
        assert!(out.contains("    #[serde(rename = \"type\")]\n    pub type_: String,"), "{out}");
        assert!(out.contains("    pub qty: f64,"), "{out}");
        assert!(out.contains("    pub items: Vec<ItemsItem>,"), "{out}");
    }

    #[test]
    fn go_and_root_array() {
        let out = generate(r#"[{"user_id": 1}, {"user_id": 2, "x-y": true}]"#, Lang::Go).unwrap();
        assert!(out.contains("type Root []RootItem"), "{out}");
        assert!(out.contains("\tUserId int64 `json:\"user_id\"`"), "{out}");
        assert!(out.contains("\tXY *bool `json:\"x-y,omitempty\"`"), "{out}");
        assert!(generate("{", Lang::Go).is_err());
    }
}
