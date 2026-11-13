# DevUtils

A native macOS app that gathers the small tools developers reach for every day — formatting JSON, decoding a JWT, testing a regex, converting a cURL command — in one fast window. Everything runs locally: nothing you paste ever leaves your machine.

Built in Rust with [egui](https://github.com/emilk/egui).

## Tools

| Category | Tools |
|---|---|
| **Date & scheduling** | Unix Time Converter · Cron Job Parser (description + next runs) |
| **Data conversion** | JSON Format/Validate · URL Parser · JSON to Code (TypeScript, Rust/serde, Go) |
| **Encoding** | Base64 String · Base64 Image (drag & drop, data URIs) · URL Encode/Decode · HTML Entities · Backslash Escape/Unescape · Hex ↔ ASCII |
| **Security & identifiers** | JWT Debugger · UUID v4 / ULID Generate & Decode · Hash Generator (MD5, SHA-1, SHA-256, SHA-512) · X.509 Certificate Decoder |
| **Text & generators** | RegExp Tester (live) · Text Diff · Lorem Ipsum · QR Code Reader/Generator · String Inspector · Case Converter · Color Converter · Random String · Line Sort/Dedupe |
| **Code & markup** | Number Base Converter · HTML, CSS, SCSS, JS and XML Beautify/Minify · SQL Formatter · Markdown Preview · cURL to Code (fetch, requests, reqwest) |

Most tools show the input on the left and the result on the right, with buttons to paste from the clipboard and copy the result.

### Keyboard shortcuts

| Shortcut | Action |
|---|---|
| ⌘1 | Show / hide the sidebar |
| ⌘⇧F | Search tools |

## Build and run

Requires macOS and a recent stable [Rust toolchain](https://rustup.rs).

```sh
cd app
cargo run                # start the app (debug build)
cargo test               # run the test suite
cargo build --release    # optimized binary: app/target/release/devutils
```

The release profile uses full link-time optimization for a smaller binary, so a release build takes a few minutes.

The interface uses the macOS system fonts (SF Pro, SF Mono), loaded from `/System/Library/Fonts` at startup. They are not bundled; on other systems egui's default fonts are used.

## Project layout

```
app/
├── Cargo.toml
├── assets/app-icon.png
└── src/
    ├── main.rs          window, sidebar, title bar, welcome screen
    ├── ui.rs            color theme, fonts, shared widgets and the input/output layout
    └── tools/
        ├── mod.rs       Tool trait, TextTool helper, tool registry
        ├── format/      beautify/minify (css.rs and html.rs are hand-written)
        └── *.rs         one file per tool or group of small tools
```

## Adding a tool

A tool that turns text into text only needs a function and one line in the registry:

```rust
// src/tools/text.rs
pub fn reverse() -> Box<dyn Tool> {
    Box::new(TextTool::new("Reverse Text", "RV", Category::Text, vec![
        ("Reverse", |s| Ok(s.chars().rev().collect())),
    ]).describe("Reverses the characters of the input."))
}
```

```rust
// src/tools/mod.rs, in registry()
text::reverse(),
```

Each action returns `Result<String, String>`: the value goes to the output pane, an error is shown in red. The first action is drawn as the primary button.

For a tool with its own interface (options, previews, several inputs), implement the `Tool` trait on a struct that holds its state, and draw it with the helpers in `ui.rs` (`split`, `toolbar`, `fill_editor`, `action_button`). `diff.rs` and `lines.rs` are short examples.
