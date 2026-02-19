# kPDF

[English](README.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="assets/app.png" alt="kPDF Icon" width="192" />
</p>

<p align="center">
  <a href="https://github.com/KusStar/kpdf/releases">
    <img src="https://img.shields.io/badge/Download-Releases-2ea44f?style=for-the-badge" alt="Download" />
  </a>
</p>

kPDF is a lightweight desktop PDF viewer built with Rust + GPUI, focused on smooth reading, fast file switching, and session restore.

## Screenshots

<img src="assets/screenshots/macos.png" alt="kPDF Screenshot on macOS" width="1024" />
<img src="assets/screenshots/settings.png" alt="kPDF Screenshot on macOS" width="1024" />
<img src="assets/screenshots/about.png" alt="kPDF Screenshot on macOS" width="1024" />

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
- Markdown notes: right-click any page position to add/edit/delete note anchors, with live preview via `TextView::markdown`
- Bilingual UI (English and Simplified Chinese), auto-detected from system locale
- Optional file logging, with menu toggles and quick-open logs folder

## Tech Stack

- Rust (Edition 2024)
- [gpui](https://crates.io/crates/gpui) / [gpui-component](https://crates.io/crates/gpui-component)
- [pdfium-render](https://crates.io/crates/pdfium-render) for PDF rendering
- [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries/releases)
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
  - Close About dialog / Command Panel / Markdown note editor
  - Clear text selection
- In Markdown note editor:
  - `Cmd/Ctrl + Enter`: Save note
- In Command Panel:
  - `Up/Down`: Move selection
  - `Enter`: Execute selected item

## Environment Variables

- `KPDF_PDFIUM_LIB_DIR`: Pdfium library directory
- `KPDF_LOG_FILE`: Custom log file path

## Data and Log Paths

### Local state database

Stores recent files, reading positions, window size, open tabs, bookmarks, and markdown notes.

- Windows: `%APPDATA%/kpdf/recent_files_db`
- macOS / Linux: `~/.kpdf/recent_files_db`

### Default log file

- Windows: `%APPDATA%/kPDF/logs/debug.log`
- macOS: `~/Library/Logs/kPDF/debug.log`
- Linux: `~/.kpdf/logs/debug.log`

### Logging state file

- Windows: `%APPDATA%/kpdf/logging_enabled`
- macOS / Linux: `~/.kpdf/logging_enabled`
