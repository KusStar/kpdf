use std::fs::{create_dir_all, read_to_string, write};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tklog::{Format, LEVEL, LOG};

static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static LOGGING_STATE_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static FILE_LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);
static FILE_HANDLER_INITIALIZED: AtomicBool = AtomicBool::new(false);

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

fn resolve_logging_state_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(app_data).join("kpdf").join("logging_enabled"));
    }

    if let Some(home) = std::env::var_os("HOME") {
        return Some(PathBuf::from(home).join(".kpdf").join("logging_enabled"));
    }

    Some(std::env::temp_dir().join("kpdf-logging-enabled"))
}

fn logging_state_path() -> Option<PathBuf> {
    LOGGING_STATE_PATH
        .get_or_init(resolve_logging_state_path)
        .clone()
}

fn persisted_logging_enabled() -> bool {
    let Some(path) = logging_state_path() else {
        return false;
    };

    let Ok(raw) = read_to_string(path) else {
        return false;
    };

    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn persist_logging_enabled(enabled: bool) {
    let Some(path) = logging_state_path() else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = create_dir_all(parent);
    }

    let _ = write(path, if enabled { "1" } else { "0" });
}

pub fn log_file_path() -> Option<PathBuf> {
    LOG_PATH.get_or_init(resolve_log_path).clone()
}

pub fn file_logging_enabled() -> bool {
    FILE_LOGGING_ENABLED.load(Ordering::Relaxed)
}

pub fn enable_file_logging() -> bool {
    if file_logging_enabled() {
        return true;
    }

    let Some(path) = LOG_PATH.get_or_init(resolve_log_path).clone() else {
        eprintln!("[log] cannot enable file logging: no writable path");
        return false;
    };

    if let Some(parent) = path.parent()
        && let Err(err) = create_dir_all(parent)
    {
        eprintln!(
            "[log] failed to create log dir: {} | {}",
            parent.display(),
            err
        );
        return false;
    }

    if !FILE_HANDLER_INITIALIZED.load(Ordering::Relaxed) {
        let path_string = path.to_string_lossy().to_string();
        LOG.set_cutmode_by_size(&path_string, 10 * 1024 * 1024, 5, true);
        FILE_HANDLER_INITIALIZED.store(true, Ordering::Relaxed);
    }

    FILE_LOGGING_ENABLED.store(true, Ordering::Relaxed);
    persist_logging_enabled(true);
    true
}

pub fn disable_file_logging() {
    FILE_LOGGING_ENABLED.store(false, Ordering::Relaxed);
    persist_logging_enabled(false);
}

pub fn initialize() {
    LOG.set_level(LEVEL::Debug)
        .set_console(true)
        .set_format(Format::LevelFlag | Format::Date | Format::Time | Format::ShortFileName)
        .set_formatter("{level}{time} {file}:{message}\n");

    if persisted_logging_enabled() {
        let _ = enable_file_logging();
    } else {
        FILE_LOGGING_ENABLED.store(false, Ordering::Relaxed);
    }
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        if $crate::logger::file_logging_enabled() {
            tklog::debug!(format!($($arg)*));
        }
    }};
}
