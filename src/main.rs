#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

pub mod i18n;
pub mod icons;
pub mod logger;
mod pdf_viewer;
mod updater;

use gpui::*;
use gpui_component::*;
use pdf_viewer::PdfViewer;

const WINDOW_SIZE_TREE: &str = "window_size";
const WINDOW_SIZE_KEY_WIDTH: &str = "width";
const WINDOW_SIZE_KEY_HEIGHT: &str = "height";
pub(crate) const APP_REPOSITORY_URL: &str = "https://github.com/KusStar/kpdf";

gpui::actions!(
    kpdf,
    [
        ShowAboutMenu,
        CheckForUpdatesMenu,
        ShowSettingsMenu,
        EnableLoggingMenu,
        DisableLoggingMenu,
        OpenLogsMenu
    ]
);

pub(crate) fn configure_app_menus(cx: &mut App, i18n: i18n::I18n) {
    let mut items = vec![
        MenuItem::action(i18n.about_button(), ShowAboutMenu),
        MenuItem::action(i18n.check_updates_button(), CheckForUpdatesMenu),
        MenuItem::action(i18n.settings_button(), ShowSettingsMenu),
        MenuItem::separator(),
    ];

    if logger::file_logging_enabled() {
        items.extend([
            MenuItem::action(i18n.open_logs_button(), OpenLogsMenu),
            MenuItem::separator(),
            MenuItem::action(i18n.disable_logging_button(), DisableLoggingMenu),
        ]);
    } else {
        items.push(MenuItem::action(
            i18n.enable_logging_button(),
            EnableLoggingMenu,
        ));
    }

    cx.set_menus(vec![Menu {
        name: "kPDF".into(),
        items,
    }]);
}

fn window_size_db_path() -> std::path::PathBuf {
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return std::path::PathBuf::from(app_data)
            .join("kpdf")
            .join("recent_files_db");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return std::path::PathBuf::from(home)
            .join(".kpdf")
            .join("recent_files_db");
    }
    std::path::PathBuf::from("kpdf_recent_files_db")
}

fn app_resources_themes_dir(current_exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let macos_dir = current_exe.parent()?;
    if macos_dir.file_name()?.to_string_lossy() != "MacOS" {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name()?.to_string_lossy() != "Contents" {
        return None;
    }

    Some(contents_dir.join("Resources").join("themes"))
}

fn push_theme_dir(
    candidates: &mut Vec<std::path::PathBuf>,
    seen: &mut std::collections::HashSet<std::path::PathBuf>,
    candidate: std::path::PathBuf,
) {
    if candidate.as_os_str().is_empty() {
        return;
    }

    let normalized = if candidate.exists() {
        candidate.canonicalize().unwrap_or(candidate)
    } else if candidate.is_relative() {
        std::env::current_dir()
            .map(|cwd| cwd.join(&candidate))
            .unwrap_or(candidate)
    } else {
        candidate
    };

    if seen.insert(normalized.clone()) {
        candidates.push(normalized);
    }
}

fn collect_theme_dirs() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(resources_themes_dir) = app_resources_themes_dir(&current_exe) {
            push_theme_dir(&mut candidates, &mut seen, resources_themes_dir);
        }

        if let Some(exe_dir) = current_exe.parent() {
            push_theme_dir(
                &mut candidates,
                &mut seen,
                exe_dir.join("assets").join("themes"),
            );
            push_theme_dir(&mut candidates, &mut seen, exe_dir.join("themes"));

            for ancestor in exe_dir.ancestors().take(6) {
                push_theme_dir(
                    &mut candidates,
                    &mut seen,
                    ancestor.join("assets").join("themes"),
                );
                push_theme_dir(&mut candidates, &mut seen, ancestor.join("themes"));
            }
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        push_theme_dir(
            &mut candidates,
            &mut seen,
            current_dir.join("assets").join("themes"),
        );
        push_theme_dir(&mut candidates, &mut seen, current_dir.join("themes"));
    }

    push_theme_dir(
        &mut candidates,
        &mut seen,
        std::path::PathBuf::from("./assets/themes"),
    );
    push_theme_dir(
        &mut candidates,
        &mut seen,
        std::path::PathBuf::from("./themes"),
    );

    candidates
}

fn dir_contains_theme_json(dir: &std::path::Path) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    entries.flatten().any(|entry| {
        let path = entry.path();
        path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
    })
}

fn themes_dir_path() -> std::path::PathBuf {
    let candidates = collect_theme_dirs();

    for candidate in &candidates {
        if dir_contains_theme_json(candidate) {
            return candidate.clone();
        }
    }

    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }

    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| std::path::PathBuf::from("assets").join("themes"))
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
    logger::initialize();

    let app = Application::new().with_assets(icons::Assets);
    let language = i18n::Language::detect();
    let i18n = i18n::I18n::new(language);

    app.run(move |cx| {
        configure_app_menus(cx, i18n);

        gpui_component::init(cx);
        let themes_dir = themes_dir_path();
        if let Err(err) = ThemeRegistry::watch_dir(themes_dir.clone(), cx, |_| {}) {
            crate::debug_log!(
                "[theme] failed to watch themes dir: {} | {}",
                themes_dir.display(),
                err
            );
        }
        Theme::change(cx.window_appearance(), None, cx);
        #[cfg(target_os = "macos")]
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        cx.spawn(async move |cx| {
            let saved_size = load_saved_window_size();
            let saved_window_bounds = if let Some((w, h)) = saved_size {
                Some(cx.update(|app| WindowBounds::centered(size(px(w), px(h)), app))?)
            } else {
                None
            };

            let window_options = WindowOptions {
                titlebar: Some(TitleBar::title_bar_options()),
                window_decorations: Some(WindowDecorations::Client),
                window_bounds: saved_window_bounds,
                ..WindowOptions::default()
            };

            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|cx| PdfViewer::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();

        cx.activate(true);
    });
}
