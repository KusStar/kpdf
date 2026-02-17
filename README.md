# kPDF

[English](README.md) | [简体中文](README.zh-CN.md)

<img src="assets/app.png" alt="kPDF Icon" width="256" />

kPDF is a lightweight desktop PDF viewer built with Rust + GPUI, focused on smooth reading, fast file switching, and session restore.

## Screenshots

<img src="assets/screenshots/macos.png" alt="kPDF Screenshot on macOS" width="1024" />

## Features

- Multi-tab reading with draggable tab reordering
- Thumbnail sidebar for quick page navigation
- Session persistence:
  - Recent files (up to 12)
  - Last read page per file
  - Open tabs and active tab restore after restart
  - Window size restore
- Command Panel for quick filtering across:
  - Open file action
  - Opened files
  - Recent files
- Text selection and copy (mouse selection, context menu, keyboard shortcuts)
- Bilingual UI (English and Simplified Chinese), auto-detected from system locale
- Optional file logging, with menu toggles and quick-open logs folder

## Tech Stack

- Rust (Edition 2024)
- [gpui](https://crates.io/crates/gpui) / [gpui-component](https://crates.io/crates/gpui-component)
- [pdfium-render](https://crates.io/crates/pdfium-render) for PDF rendering
- [sled](https://crates.io/crates/sled) for local state persistence

## Requirements

1. Install Rust (latest stable recommended)
2. Provide a Pdfium dynamic library for your platform:
   - macOS: `libpdfium.dylib`
   - Linux: `libpdfium.so`
   - Windows: `pdfium.dll`

kPDF searches Pdfium in this order:

1. Directory from `KPDF_PDFIUM_LIB_DIR`
2. App resources and `lib` near the executable
3. Current working directory and `./lib`
4. System library path

This repository already includes the macOS library at `lib/libpdfium.dylib`.

### Linux clipboard dependency

On Linux, copy uses `xclip` first and falls back to `wl-copy`. Install at least one of them.

## Quick Start

```bash
# Run in development mode
cargo run

# Run in release mode
cargo run --release

# Build release binary
cargo build --release
```

## Keyboard Shortcuts

- `Cmd/Ctrl + O`: Open PDF
- `Cmd/Ctrl + W`: Close current tab
- `Cmd/Ctrl + T`: Toggle Command Panel
- `Cmd/Ctrl + Shift + [`: Previous tab
- `Cmd/Ctrl + Shift + ]`: Next tab
- `Cmd/Ctrl + 1..9`: Jump to tab (`9` = last tab)
- `Cmd/Ctrl + A`: Select all text on current page (if page text is loaded)
- `Cmd/Ctrl + C`: Copy selected text
- `Esc`:
  - Close About dialog / Command Panel
  - Clear text selection
- In Command Panel:
  - `Up/Down`: Move selection
  - `Enter`: Execute selected item

## Environment Variables

- `KPDF_LANG`: Force UI language (e.g. `zh_CN`, `en_US`)
- `KPDF_PDFIUM_LIB_DIR`: Pdfium library directory
- `KPDF_LOG_FILE`: Custom log file path

## Data and Log Paths

### Local state database

Stores recent files, reading positions, window size, and open tabs.

- Windows: `%APPDATA%/kpdf/recent_files_db`
- macOS / Linux: `~/.kpdf/recent_files_db`

### Default log file

- Windows: `%APPDATA%/kPDF/logs/debug.log`
- macOS: `~/Library/Logs/kPDF/debug.log`
- Linux: `~/.kpdf/logs/debug.log`

### Logging state file

- Windows: `%APPDATA%/kpdf/logging_enabled`
- macOS / Linux: `~/.kpdf/logging_enabled`

## Project Structure

```text
.
├── assets/                  # App assets and icons
├── lib/                     # Pdfium dynamic library (macOS included)
├── src/
│   ├── main.rs              # Application entry point
│   ├── pdf_viewer.rs        # Core viewer and interaction logic
│   ├── display_list.rs      # Main page display list
│   ├── thumbnail_list.rs    # Thumbnail list
│   ├── command_panel.rs     # Command Panel
│   ├── text_selection.rs    # Text selection and copy
│   ├── tab.rs               # Tab management
│   ├── i18n.rs              # Localization strings
│   └── logger.rs            # Logging setup and persistence
└── Cargo.toml
```
