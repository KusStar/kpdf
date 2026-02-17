use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::OnceLock;
use tklog::{Format, LEVEL, LOG};

static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

fn resolve_log_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("KPDF_LOG_FILE") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return Some(
            PathBuf::from(app_data)
                .join("kPDF")
                .join("logs")
                .join("debug.log"),
        );
    }

    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return Some(
            PathBuf::from(home)
                .join("Library")
                .join("Logs")
                .join("kPDF")
                .join("debug.log"),
        );
    }

    if let Some(home) = std::env::var_os("HOME") {
        return Some(
            PathBuf::from(home)
                .join(".kpdf")
                .join("logs")
                .join("debug.log"),
        );
    }

    Some(std::env::temp_dir().join("kpdf-debug.log"))
}

pub fn log_file_path() -> Option<PathBuf> {
    LOG_PATH.get_or_init(resolve_log_path).clone()
}

pub fn initialize() {
    LOG.set_level(LEVEL::Debug)
        .set_console(true)
        .set_format(Format::LevelFlag | Format::Date | Format::Time | Format::ShortFileName)
        .set_formatter("{level}{time} {file}:{message}\n");

    if let Some(path) = LOG_PATH.get_or_init(resolve_log_path).clone() {
        if let Some(parent) = path.parent() {
            if let Err(err) = create_dir_all(parent) {
                tklog::warn!(format!(
                    "[log] failed to create log dir: {} | {}",
                    parent.display(),
                    err
                ));
                return;
            }
        }

        let path_string = path.to_string_lossy().to_string();
        LOG.set_cutmode_by_size(&path_string, 10 * 1024 * 1024, 5, true);
        tklog::info!(format!("[log] file path: {}", path.display()));
    } else {
        tklog::warn!("[log] file logging disabled: no writable path");
    }
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        tklog::debug!(format!($($arg)*));
    }};
}
