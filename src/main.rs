pub mod i18n;
pub mod icons;
mod pdf_viewer;

use gpui::*;
use gpui_component::*;
use pdf_viewer::PdfViewer;

const WINDOW_SIZE_TREE: &str = "window_size";
const WINDOW_SIZE_KEY_WIDTH: &str = "width";
const WINDOW_SIZE_KEY_HEIGHT: &str = "height";

fn window_size_db_path() -> std::path::PathBuf {
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return std::path::PathBuf::from(app_data).join("kpdf").join("recent_files_db");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return std::path::PathBuf::from(home).join(".kpdf").join("recent_files_db");
    }
    std::path::PathBuf::from("kpdf_recent_files_db")
}

fn load_saved_window_size() -> Option<(f32, f32)> {
    let db_path = window_size_db_path();
    let db = match sled::open(&db_path) {
        Ok(db) => db,
        Err(_) => return None,
    };
    let store = match db.open_tree(WINDOW_SIZE_TREE) {
        Ok(tree) => tree,
        Err(_) => return None,
    };
    let width_bytes = store.get(WINDOW_SIZE_KEY_WIDTH).ok().flatten()?;
    let height_bytes = store.get(WINDOW_SIZE_KEY_HEIGHT).ok().flatten()?;
    if width_bytes.len() != 4 || height_bytes.len() != 4 {
        return None;
    }
    let width = f32::from_be_bytes(width_bytes.as_ref().try_into().ok()?);
    let height = f32::from_be_bytes(height_bytes.as_ref().try_into().ok()?);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some((width, height))
}

fn main() {
    let app = Application::new().with_assets(icons::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);
        Theme::change(cx.window_appearance(), None, cx);

        cx.spawn(async move |cx| {
            let saved_size = load_saved_window_size();

            let window_options = WindowOptions {
                titlebar: Some(TitleBar::title_bar_options()),
                window_decorations: Some(WindowDecorations::Client),
                window_bounds: saved_size.map(|(w, h)| {
                    WindowBounds::Windowed(Bounds::new(
                        point(px(100.0), px(100.0)),
                        size(px(w), px(h)),
                    ))
                }),
                ..WindowOptions::default()
            };

            cx.open_window(window_options, |_, cx| cx.new(|_| PdfViewer::new()))?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();

        cx.activate(true);
    });
}
