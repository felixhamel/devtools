<p align="center">
  <img src="app/assets/app-icon.png" width="128" height="128" alt="DevUtils icon">
</p>

<h1 align="center">DevUtils</h1>

<p align="center">
  Everyday developer tools in one fast, native macOS window.<br>
  Built in Rust with <a href="https://github.com/emilk/egui">egui</a>.
</p>

A native macOS app that gathers the small tools developers reach for every day — formatting JSON, decoding a JWT, testing a regex, converting a cURL command — in one fast window. Everything runs locally: nothing you paste ever leaves your machine.

## Tools

| Category | Tools |
|---|---|
| **Date & scheduling** | Unix Time Converter · Cron Job Parser (description + next runs) |
| **Data conversion** | JSON Format/Validate · URL Parser · JSON to Code (TypeScript, Rust/serde, Go) |
| **Encoding** | Base64 String · Base64 Image (drag & drop, data URIs) · URL Encode/Decode · HTML Entities · Backslash Escape/Unescape · Hex ↔ ASCII |
| **Security & identifiers** | JWT Debugger · UUID v4 / ULID Generate & Decode · Hash Generator (MD5, SHA-1, SHA-256, SHA-512) · X.509 Certificate Decoder |
| **Text & generators** | RegExp Tester (live) · Text Diff · Lorem Ipsum · QR Code Reader/Generator · String Inspector · Case Converter · Color Converter · Random String · Line Sort/Dedupe |
| **Code & markup** | Number Base Converter · HTML, CSS, SCSS, JS and XML Beautify/Minify · SQL Formatter · Markdown Preview · cURL to Code (fetch, requests, reqwest) |

Most tools show the input on the left and the result on the right, with syntax highlighting for code (JSON, SQL, JS, HTML, CSS, XML, Markdown…), a button to paste from the clipboard, a **Sample** button that loads an example, and a Copy button.

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

### Install as a macOS app

```sh
scripts/bundle-macos.sh             # builds dist/DevUtils.app and a release zip
scripts/bundle-macos.sh --install   # ...and copies DevUtils.app to /Applications
```

Release builds are also attached to the [GitHub releases](../../releases). The app is signed ad hoc, not notarized: after downloading it, allow it once with right-click → **Open**, or run `xattr -dr com.apple.quarantine /Applications/DevUtils.app`.

The interface uses the macOS system fonts (SF Pro, SF Mono), loaded from `/System/Library/Fonts` at startup. They are not bundled; on other systems egui's default fonts are used. Icons come from [Phosphor](https://phosphoricons.com) via `egui-phosphor`.

### Command-line options

| Option | Effect |
|---|---|
| `--tool "<name>"` | Open a tool directly, e.g. `--tool "JSON Format/Validate"` |
| `--sample` | With `--tool`, load the tool's example |
| `--screenshot <file.png>` | Save a screenshot of the window and quit (handy to check the interface) |

```sh
cargo run -- --tool "SQL Formatter" --sample --screenshot /tmp/sql.png
```

## Project layout

```
app/
├── Cargo.toml
├── assets/app-icon.png
└── src/
    ├── main.rs          window, sidebar, title bar, welcome screen
    ├── ui.rs            color theme, fonts, icons, shared widgets, editors and highlighting
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
    Box::new(TextTool::new("Reverse Text", icon::ARROW_COUNTER_CLOCKWISE, Category::Text, vec![
        ("Reverse", |s| Ok(s.chars().rev().collect())),
    ])
    .describe("Reverses the characters of the input.")
    .sample("Hello, world!"))
}
```

```rust
// src/tools/mod.rs, in registry()
text::reverse(),
```

Each action returns `Result<String, String>`: the value goes to the output pane, an error is shown in a red banner. The first action is drawn as the primary button. The icon is any [Phosphor](https://phosphoricons.com) icon from `ui::icon`.

Optional builder methods:

| Method | Effect |
|---|---|
| `.describe("…")` | One-line explanation under the title |
| `.sample("…")` | Adds a Sample button that loads this input and runs the primary action |
| `.syntax("json")` | Highlights both panes (any syntect language name or extension: `sql`, `js`, `html`, `rs`…) |
| `.output_syntax(&["js", "py"])` | A different output language per action |
| `.key_value_output()` | Shows `Key: value` results with muted keys |

For a tool with its own interface (options, previews, several inputs), implement the `Tool` trait on a struct that holds its state, and draw it with the helpers in `ui.rs` (`split`, `toolbar`, `code_editor`, `action_button`, `copy_button`…). `diff.rs` and `lines.rs` are short examples.
