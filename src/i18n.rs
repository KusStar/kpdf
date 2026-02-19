use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    ZhCn,
    EnUs,
}

impl Language {
    pub fn detect() -> Self {
        for raw in [
            std::env::var("KPDF_LANG").ok(),
            std::env::var("LC_ALL").ok(),
            std::env::var("LC_MESSAGES").ok(),
            std::env::var("LANG").ok(),
        ]
        .into_iter()
        .flatten()
        {
            let tag = raw.trim().to_ascii_lowercase();
            if tag.is_empty() {
                continue;
            }

            let is_chinese = tag.starts_with("zh")
                || tag == "cn"
                || tag.starts_with("cn_")
                || tag.starts_with("cn-")
                || tag.contains("_zh")
                || tag.contains("-zh");
            if is_chinese {
                return Self::ZhCn;
            }

            return Self::EnUs;
        }

        Self::EnUs
    }

    fn file_name(self) -> &'static str {
        match self {
            Self::ZhCn => "zh_CN.json",
            Self::EnUs => "en_US.json",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct I18n {
    lang: Language,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocaleMessages {
    file_not_opened: String,
    open_button: String,
    choose_file_button: String,
    no_recent_files: String,
    last_seen_page: String,
    zoom_reset_button: String,
    add_bookmark_button: String,
    bookmark_scope_current_pdf: String,
    bookmark_scope_all: String,
    no_bookmarks: String,
    bookmark_page_label: String,
    bookmark_added_unknown: String,
    bookmark_added_relative_just_now: String,
    bookmark_added_relative_minutes: String,
    bookmark_added_relative_hours: String,
    bookmark_added_relative_days: String,
    open_logs_button: String,
    enable_logging_button: String,
    disable_logging_button: String,
    about_button: String,
    check_updates_button: String,
    settings_button: String,
    about_dialog_title: String,
    about_app_info: String,
    version_label: String,
    website_label: String,
    updates_label: String,
    update_status_idle: String,
    update_status_checking: String,
    update_status_up_to_date: String,
    update_status_available: String,
    update_status_failed: String,
    download_update_button: String,
    open_website_button: String,
    close_button: String,
    settings_dialog_title: String,
    settings_theme_section: String,
    settings_theme_label: String,
    settings_theme_hint: String,
    settings_theme_color_label: String,
    settings_theme_color_hint: String,
    settings_theme_color_placeholder: String,
    settings_theme_light: String,
    settings_theme_dark: String,
    settings_titlebar_section: String,
    settings_titlebar_navigation_label: String,
    settings_titlebar_navigation_hint: String,
    settings_titlebar_zoom_label: String,
    settings_titlebar_zoom_hint: String,
    no_pages: String,
    no_document_hint: String,
    page_render_failed: String,
    thumbnail_render_failed: String,
    open_pdf_prompt: String,
    command_panel_title: String,
    command_panel_search_hint: String,
    command_panel_open_files: String,
    command_panel_recent_files: String,
    command_panel_no_open_files: String,
    command_panel_current_badge: String,
    command_panel_menu_badge: String,
    command_panel_open_about_hint: String,
    command_panel_check_updates_hint: String,
    command_panel_open_settings_hint: String,
    command_panel_open_logs_hint: String,
    command_panel_enable_logging_hint: String,
    command_panel_disable_logging_hint: String,
    pdfium_not_found: String,
    cannot_open_file: String,
    pdfium_cache_lock_poisoned: String,
    pdfium_cannot_open_file: String,
    invalid_bitmap_size: String,
    bitmap_len_mismatch: String,
    copy_button: String,
    close_all_tabs_button: String,
    close_other_tabs_button: String,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    reveal_in_file_manager_finder: String,
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    reveal_in_file_manager_explorer: String,
    reveal_in_file_manager_default: String,
    cannot_create_image_buffer: String,
}

static ZH_CN_MESSAGES: OnceLock<LocaleMessages> = OnceLock::new();
static EN_US_MESSAGES: OnceLock<LocaleMessages> = OnceLock::new();

impl I18n {
    pub fn new(lang: Language) -> Self {
        Self { lang }
    }

    fn messages(self) -> &'static LocaleMessages {
        match self.lang {
            Language::ZhCn => ZH_CN_MESSAGES.get_or_init(|| load_messages(Language::ZhCn)),
            Language::EnUs => EN_US_MESSAGES.get_or_init(|| load_messages(Language::EnUs)),
        }
    }

    pub fn file_not_opened(self) -> &'static str {
        self.messages().file_not_opened.as_str()
    }

    pub fn open_button(self) -> &'static str {
        self.messages().open_button.as_str()
    }

    pub fn choose_file_button(self) -> &'static str {
        self.messages().choose_file_button.as_str()
    }

    pub fn no_recent_files(self) -> &'static str {
        self.messages().no_recent_files.as_str()
    }

    pub fn last_seen_page(self, page_num: usize) -> String {
        render1(
            self.messages().last_seen_page.as_str(),
            "page_num",
            page_num,
        )
    }

    pub fn zoom_reset_button(self) -> &'static str {
        self.messages().zoom_reset_button.as_str()
    }

    pub fn add_bookmark_button(self) -> &'static str {
        self.messages().add_bookmark_button.as_str()
    }

    pub fn bookmark_scope_current_pdf(self) -> &'static str {
        self.messages().bookmark_scope_current_pdf.as_str()
    }

    pub fn bookmark_scope_all(self) -> &'static str {
        self.messages().bookmark_scope_all.as_str()
    }

    pub fn no_bookmarks(self) -> &'static str {
        self.messages().no_bookmarks.as_str()
    }

    pub fn bookmark_page_label(self, page_num: usize) -> String {
        render1(
            self.messages().bookmark_page_label.as_str(),
            "page_num",
            page_num,
        )
    }

    pub fn bookmark_added_unknown(self) -> &'static str {
        self.messages().bookmark_added_unknown.as_str()
    }

    pub fn bookmark_added_relative(self, seconds_ago: u64) -> String {
        let messages = self.messages();
        if seconds_ago < 60 {
            return messages.bookmark_added_relative_just_now.clone();
        }
        if seconds_ago < 3_600 {
            return render1(
                messages.bookmark_added_relative_minutes.as_str(),
                "minutes",
                seconds_ago / 60,
            );
        }
        if seconds_ago < 86_400 {
            return render1(
                messages.bookmark_added_relative_hours.as_str(),
                "hours",
                seconds_ago / 3_600,
            );
        }
        render1(
            messages.bookmark_added_relative_days.as_str(),
            "days",
            seconds_ago / 86_400,
        )
    }

    pub fn open_logs_button(self) -> &'static str {
        self.messages().open_logs_button.as_str()
    }

    pub fn enable_logging_button(self) -> &'static str {
        self.messages().enable_logging_button.as_str()
    }

    pub fn disable_logging_button(self) -> &'static str {
        self.messages().disable_logging_button.as_str()
    }

    pub fn about_button(self) -> &'static str {
        self.messages().about_button.as_str()
    }

    pub fn check_updates_button(self) -> &'static str {
        self.messages().check_updates_button.as_str()
    }

    pub fn settings_button(self) -> &'static str {
        self.messages().settings_button.as_str()
    }

    pub fn about_dialog_title(self) -> &'static str {
        self.messages().about_dialog_title.as_str()
    }

    pub fn about_app_info(self) -> &'static str {
        self.messages().about_app_info.as_str()
    }

    pub fn version_label(self) -> &'static str {
        self.messages().version_label.as_str()
    }

    pub fn website_label(self) -> &'static str {
        self.messages().website_label.as_str()
    }

    pub fn updates_label(self) -> &'static str {
        self.messages().updates_label.as_str()
    }

    pub fn update_status_idle(self) -> &'static str {
        self.messages().update_status_idle.as_str()
    }

    pub fn update_status_checking(self) -> &'static str {
        self.messages().update_status_checking.as_str()
    }

    pub fn update_status_up_to_date(self, version: &str) -> String {
        render1(
            self.messages().update_status_up_to_date.as_str(),
            "version",
            version,
        )
    }

    pub fn update_status_available(self, version: &str) -> String {
        render1(
            self.messages().update_status_available.as_str(),
            "version",
            version,
        )
    }

    pub fn update_status_failed(self, message: &str) -> String {
        render1(
            self.messages().update_status_failed.as_str(),
            "message",
            message,
        )
    }

    pub fn download_update_button(self) -> &'static str {
        self.messages().download_update_button.as_str()
    }

    pub fn open_website_button(self) -> &'static str {
        self.messages().open_website_button.as_str()
    }

    pub fn close_button(self) -> &'static str {
        self.messages().close_button.as_str()
    }

    pub fn settings_dialog_title(self) -> &'static str {
        self.messages().settings_dialog_title.as_str()
    }

    pub fn settings_theme_section(self) -> &'static str {
        self.messages().settings_theme_section.as_str()
    }

    pub fn settings_theme_label(self) -> &'static str {
        self.messages().settings_theme_label.as_str()
    }

    pub fn settings_theme_hint(self) -> &'static str {
        self.messages().settings_theme_hint.as_str()
    }

    pub fn settings_theme_color_label(self) -> &'static str {
        self.messages().settings_theme_color_label.as_str()
    }

    pub fn settings_theme_color_hint(self) -> &'static str {
        self.messages().settings_theme_color_hint.as_str()
    }

    pub fn settings_theme_color_placeholder(self) -> &'static str {
        self.messages().settings_theme_color_placeholder.as_str()
    }

    pub fn settings_theme_light(self) -> &'static str {
        self.messages().settings_theme_light.as_str()
    }

    pub fn settings_theme_dark(self) -> &'static str {
        self.messages().settings_theme_dark.as_str()
    }

    pub fn settings_titlebar_section(self) -> &'static str {
        self.messages().settings_titlebar_section.as_str()
    }

    pub fn settings_titlebar_navigation_label(self) -> &'static str {
        self.messages().settings_titlebar_navigation_label.as_str()
    }

    pub fn settings_titlebar_navigation_hint(self) -> &'static str {
        self.messages().settings_titlebar_navigation_hint.as_str()
    }

    pub fn settings_titlebar_zoom_label(self) -> &'static str {
        self.messages().settings_titlebar_zoom_label.as_str()
    }

    pub fn settings_titlebar_zoom_hint(self) -> &'static str {
        self.messages().settings_titlebar_zoom_hint.as_str()
    }

    pub fn no_pages(self) -> &'static str {
        self.messages().no_pages.as_str()
    }

    pub fn no_document_hint(self) -> &'static str {
        self.messages().no_document_hint.as_str()
    }

    pub fn page_render_failed(self) -> &'static str {
        self.messages().page_render_failed.as_str()
    }

    pub fn thumbnail_render_failed(self) -> &'static str {
        self.messages().thumbnail_render_failed.as_str()
    }

    pub fn open_pdf_prompt(self) -> &'static str {
        self.messages().open_pdf_prompt.as_str()
    }

    pub fn command_panel_title(self) -> &'static str {
        self.messages().command_panel_title.as_str()
    }

    pub fn command_panel_search_hint(self) -> &'static str {
        self.messages().command_panel_search_hint.as_str()
    }

    pub fn command_panel_open_files(self) -> &'static str {
        self.messages().command_panel_open_files.as_str()
    }

    pub fn command_panel_recent_files(self) -> &'static str {
        self.messages().command_panel_recent_files.as_str()
    }

    pub fn command_panel_no_open_files(self) -> &'static str {
        self.messages().command_panel_no_open_files.as_str()
    }

    pub fn command_panel_current_badge(self) -> &'static str {
        self.messages().command_panel_current_badge.as_str()
    }

    pub fn command_panel_menu_badge(self) -> &'static str {
        self.messages().command_panel_menu_badge.as_str()
    }

    pub fn command_panel_open_about_hint(self) -> &'static str {
        self.messages().command_panel_open_about_hint.as_str()
    }

    pub fn command_panel_check_updates_hint(self) -> &'static str {
        self.messages().command_panel_check_updates_hint.as_str()
    }

    pub fn command_panel_open_settings_hint(self) -> &'static str {
        self.messages().command_panel_open_settings_hint.as_str()
    }

    pub fn command_panel_open_logs_hint(self) -> &'static str {
        self.messages().command_panel_open_logs_hint.as_str()
    }

    pub fn command_panel_enable_logging_hint(self) -> &'static str {
        self.messages().command_panel_enable_logging_hint.as_str()
    }

    pub fn command_panel_disable_logging_hint(self) -> &'static str {
        self.messages().command_panel_disable_logging_hint.as_str()
    }

    pub fn pdfium_not_found(self) -> &'static str {
        self.messages().pdfium_not_found.as_str()
    }

    pub fn cannot_open_file(self, path: &Path) -> String {
        render1(
            self.messages().cannot_open_file.as_str(),
            "path",
            path.to_string_lossy(),
        )
    }

    pub fn pdfium_cache_lock_poisoned(self) -> &'static str {
        self.messages().pdfium_cache_lock_poisoned.as_str()
    }

    pub fn pdfium_cannot_open_file(self, path: &Path) -> String {
        render1(
            self.messages().pdfium_cannot_open_file.as_str(),
            "path",
            path.to_string_lossy(),
        )
    }

    pub fn invalid_bitmap_size(self, width: u32, height: u32) -> String {
        render2(
            self.messages().invalid_bitmap_size.as_str(),
            "width",
            width,
            "height",
            height,
        )
    }

    pub fn bitmap_len_mismatch(self, got: usize, expected: usize) -> String {
        render2(
            self.messages().bitmap_len_mismatch.as_str(),
            "got",
            got,
            "expected",
            expected,
        )
    }

    pub fn copy_button(self) -> &'static str {
        self.messages().copy_button.as_str()
    }

    pub fn close_all_tabs_button(self) -> &'static str {
        self.messages().close_all_tabs_button.as_str()
    }

    pub fn close_other_tabs_button(self) -> &'static str {
        self.messages().close_other_tabs_button.as_str()
    }

    pub fn reveal_in_file_manager_button(self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            return self.messages().reveal_in_file_manager_finder.as_str();
        }
        #[cfg(target_os = "windows")]
        {
            return self.messages().reveal_in_file_manager_explorer.as_str();
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            return self.messages().reveal_in_file_manager_default.as_str();
        }
        #[allow(unreachable_code)]
        self.messages().reveal_in_file_manager_default.as_str()
    }

    pub fn cannot_create_image_buffer(self, width: u32, height: u32) -> String {
        render2(
            self.messages().cannot_create_image_buffer.as_str(),
            "width",
            width,
            "height",
            height,
        )
    }
}

fn load_messages(lang: Language) -> LocaleMessages {
    match try_load_messages(lang) {
        Ok(messages) => messages,
        Err(primary_err) => {
            crate::debug_log!(
                "[i18n] failed to load {}: {}",
                lang.file_name(),
                primary_err
            );

            if lang == Language::EnUs {
                panic!(
                    "failed to load i18n file {}: {}",
                    lang.file_name(),
                    primary_err
                );
            }

            match try_load_messages(Language::EnUs) {
                Ok(messages) => {
                    crate::debug_log!(
                        "[i18n] fallback to {} after {} failed",
                        Language::EnUs.file_name(),
                        lang.file_name()
                    );
                    messages
                }
                Err(fallback_err) => panic!(
                    "failed to load i18n files {} ({}) and {} ({})",
                    lang.file_name(),
                    primary_err,
                    Language::EnUs.file_name(),
                    fallback_err
                ),
            }
        }
    }
}

fn try_load_messages(lang: Language) -> Result<LocaleMessages, String> {
    let (path, raw) = load_locale_file(lang.file_name())?;
    crate::debug_log!(
        "[i18n] loading locale {} from {}",
        lang.file_name(),
        path.display()
    );
    serde_json::from_str::<LocaleMessages>(&raw)
        .map_err(|err| format!("{} parse failed: {}", path.display(), err))
}

fn load_locale_file(file_name: &str) -> Result<(PathBuf, String), String> {
    let candidates = collect_i18n_dirs();
    for dir in &candidates {
        let path = dir.join(file_name);
        if !path.is_file() {
            continue;
        }

        let raw = std::fs::read_to_string(&path)
            .map_err(|err| format!("{} read failed: {}", path.display(), err))?;
        return Ok((path, raw));
    }

    let searched = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "{} not found; searched directories: {}",
        file_name, searched
    ))
}

fn app_resources_i18n_dir(current_exe: &Path) -> Option<PathBuf> {
    let macos_dir = current_exe.parent()?;
    if macos_dir.file_name()?.to_string_lossy() != "MacOS" {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name()?.to_string_lossy() != "Contents" {
        return None;
    }

    Some(contents_dir.join("Resources").join("i18n"))
}

fn push_i18n_dir(
    candidates: &mut Vec<PathBuf>,
    seen: &mut std::collections::HashSet<PathBuf>,
    candidate: PathBuf,
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

fn collect_i18n_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(resources_i18n_dir) = app_resources_i18n_dir(&current_exe) {
            push_i18n_dir(&mut candidates, &mut seen, resources_i18n_dir);
        }

        if let Some(exe_dir) = current_exe.parent() {
            push_i18n_dir(
                &mut candidates,
                &mut seen,
                exe_dir.join("assets").join("i18n"),
            );
            push_i18n_dir(&mut candidates, &mut seen, exe_dir.join("i18n"));

            for ancestor in exe_dir.ancestors().take(6) {
                push_i18n_dir(
                    &mut candidates,
                    &mut seen,
                    ancestor.join("assets").join("i18n"),
                );
                push_i18n_dir(&mut candidates, &mut seen, ancestor.join("i18n"));
            }
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        push_i18n_dir(
            &mut candidates,
            &mut seen,
            current_dir.join("assets").join("i18n"),
        );
        push_i18n_dir(&mut candidates, &mut seen, current_dir.join("i18n"));
    }

    push_i18n_dir(&mut candidates, &mut seen, PathBuf::from("./assets/i18n"));
    push_i18n_dir(&mut candidates, &mut seen, PathBuf::from("./i18n"));

    candidates
}

fn render1(template: &str, key: &str, value: impl std::fmt::Display) -> String {
    let token = format!("{{{key}}}");
    template.replace(&token, &value.to_string())
}

fn render2(
    template: &str,
    key1: &str,
    value1: impl std::fmt::Display,
    key2: &str,
    value2: impl std::fmt::Display,
) -> String {
    let once = render1(template, key1, value1);
    render1(&once, key2, value2)
}
