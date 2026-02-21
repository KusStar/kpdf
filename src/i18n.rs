use serde::Deserialize;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    ZhCn,
    EnUs,
}

impl Language {
    pub fn detect() -> Self {
        if let Some(locale_tag) = sys_locale::get_locale() {
            return Self::from_locale_tag(&locale_tag);
        }

        Self::EnUs
    }

    fn from_locale_tag(raw: &str) -> Self {
        let tag = raw.trim().to_ascii_lowercase();
        if tag.is_empty() {
            return Self::EnUs;
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

        Self::EnUs
    }

    fn file_name(self) -> &'static str {
        match self {
            Self::ZhCn => "zh_CN.json",
            Self::EnUs => "en_US.json",
        }
    }
}

macro_rules! locale_message_fields {
    ($macro:ident) => {
        $macro! {
            file_not_opened,
            open_button,
            choose_file_button,
            no_recent_files,
            last_seen_page,
            zoom_reset_button,
            add_bookmark_button,
            bookmark_scope_current_pdf,
            bookmark_scope_all,
            no_bookmarks,
            bookmark_page_label,
            bookmark_added_unknown,
            bookmark_added_relative_just_now,
            bookmark_added_relative_minutes,
            bookmark_added_relative_hours,
            bookmark_added_relative_days,
            bookmark_notes_count_label,
            open_logs_button,
            enable_logging_button,
            disable_logging_button,
            about_button,
            check_updates_button,
            settings_button,
            about_dialog_title,
            about_app_info,
            version_label,
            website_label,
            updates_label,
            update_status_idle,
            update_status_checking,
            update_status_up_to_date,
            update_status_available,
            update_status_failed,
            download_update_button,
            open_website_button,
            close_button,
            settings_dialog_title,
            settings_language_section,
            settings_language_label,
            settings_language_hint,
            settings_language_system,
            settings_language_zh_cn,
            settings_language_en_us,
            settings_db_section,
            settings_db_usage_label,
            settings_db_usage_hint,
            settings_db_path_label,
            settings_db_refresh_button,
            settings_theme_section,
            settings_theme_label,
            settings_theme_hint,
            settings_theme_color_label,
            settings_theme_color_hint,
            settings_theme_color_placeholder,
            settings_theme_light,
            settings_theme_dark,
            settings_titlebar_section,
            settings_titlebar_navigation_label,
            settings_titlebar_navigation_hint,
            settings_titlebar_zoom_label,
            settings_titlebar_zoom_hint,
            no_pages,
            no_document_hint,
            page_render_failed,
            thumbnail_render_failed,
            open_pdf_prompt,
            command_panel_title,
            command_panel_search_hint,
            command_panel_open_files,
            command_panel_recent_files,
            command_panel_no_open_files,
            command_panel_current_badge,
            command_panel_menu_badge,
            command_panel_open_about_hint,
            command_panel_check_updates_hint,
            command_panel_open_settings_hint,
            command_panel_open_logs_hint,
            command_panel_enable_logging_hint,
            command_panel_disable_logging_hint,
            pdfium_not_found,
            cannot_open_file,
            pdfium_cache_lock_poisoned,
            pdfium_cannot_open_file,
            invalid_bitmap_size,
            bitmap_len_mismatch,
            copy_button,
            text_markup_highlight_button,
            text_markup_underline_button,
            text_markup_add_note_button,
            text_markup_reset_button,
            add_note_here_button,
            edit_note_button,
            delete_note_button,
            delete_highlight_button,
            copy_note_button,
            note_new_dialog_title,
            note_edit_dialog_title,
            note_dialog_hint,
            note_input_placeholder,
            note_show_preview_button,
            note_hide_preview_button,
            note_save_button,
            note_cancel_button,
            close_all_tabs_button,
            close_other_tabs_button,
            reveal_in_file_manager_finder,
            reveal_in_file_manager_explorer,
            reveal_in_file_manager_default,
            cannot_create_image_buffer,
        }
    };
}

macro_rules! define_raw_locale_messages {
    ($($field:ident),+ $(,)?) => {
        #[derive(Debug, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawLocaleMessages {
            $(
                $field: String,
            )+
        }
    };
}

macro_rules! define_locale_messages {
    ($($field:ident),+ $(,)?) => {
        #[derive(Debug)]
        pub struct LocaleMessages {
            $(
                pub $field: &'static str,
            )+
        }
    };
}

macro_rules! impl_from_raw_locale_messages {
    ($($field:ident),+ $(,)?) => {
        impl From<RawLocaleMessages> for LocaleMessages {
            fn from(raw: RawLocaleMessages) -> Self {
                Self {
                    $(
                        $field: leak_str(raw.$field),
                    )+
                }
            }
        }
    };
}

locale_message_fields!(define_raw_locale_messages);
locale_message_fields!(define_locale_messages);
locale_message_fields!(impl_from_raw_locale_messages);

fn leak_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

static ZH_CN_MESSAGES: OnceLock<LocaleMessages> = OnceLock::new();
static EN_US_MESSAGES: OnceLock<LocaleMessages> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
pub struct I18n {
    messages: &'static LocaleMessages,
    pub reveal_in_file_manager_button: &'static str,
}

impl I18n {
    pub fn new(lang: Language) -> Self {
        let messages = messages_for(lang);
        let reveal_in_file_manager_button = reveal_button(messages);

        Self {
            messages,
            reveal_in_file_manager_button,
        }
    }

    pub fn last_seen_page(self, page_num: usize) -> String {
        format_template(self.last_seen_page, &[("page_num", page_num.to_string())])
    }

    pub fn bookmark_page_label(self, page_num: usize) -> String {
        format_template(
            self.bookmark_page_label,
            &[("page_num", page_num.to_string())],
        )
    }

    pub fn bookmark_added_relative(self, seconds_ago: u64) -> String {
        if seconds_ago < 60 {
            return self.bookmark_added_relative_just_now.to_string();
        }
        if seconds_ago < 3_600 {
            return format_template(
                self.bookmark_added_relative_minutes,
                &[("minutes", (seconds_ago / 60).to_string())],
            );
        }
        if seconds_ago < 86_400 {
            return format_template(
                self.bookmark_added_relative_hours,
                &[("hours", (seconds_ago / 3_600).to_string())],
            );
        }
        format_template(
            self.bookmark_added_relative_days,
            &[("days", (seconds_ago / 86_400).to_string())],
        )
    }

    pub fn bookmark_notes_count_label(self, count: usize) -> String {
        format_template(
            self.bookmark_notes_count_label,
            &[("count", count.to_string())],
        )
    }

    pub fn update_status_up_to_date(self, version: &str) -> String {
        format_template(
            self.update_status_up_to_date,
            &[("version", version.to_string())],
        )
    }

    pub fn update_status_available(self, version: &str) -> String {
        format_template(
            self.update_status_available,
            &[("version", version.to_string())],
        )
    }

    pub fn update_status_failed(self, message: &str) -> String {
        format_template(
            self.update_status_failed,
            &[("message", message.to_string())],
        )
    }

    pub fn cannot_open_file(self, path: &Path) -> String {
        format_template(
            self.cannot_open_file,
            &[("path", path.to_string_lossy().to_string())],
        )
    }

    pub fn pdfium_cannot_open_file(self, path: &Path) -> String {
        format_template(
            self.pdfium_cannot_open_file,
            &[("path", path.to_string_lossy().to_string())],
        )
    }

    pub fn invalid_bitmap_size(self, width: u32, height: u32) -> String {
        format_template(
            self.invalid_bitmap_size,
            &[("width", width.to_string()), ("height", height.to_string())],
        )
    }

    pub fn bitmap_len_mismatch(self, got: usize, expected: usize) -> String {
        format_template(
            self.bitmap_len_mismatch,
            &[("got", got.to_string()), ("expected", expected.to_string())],
        )
    }

    pub fn cannot_create_image_buffer(self, width: u32, height: u32) -> String {
        format_template(
            self.cannot_create_image_buffer,
            &[("width", width.to_string()), ("height", height.to_string())],
        )
    }
}

impl Deref for I18n {
    type Target = LocaleMessages;

    fn deref(&self) -> &Self::Target {
        self.messages
    }
}

fn messages_for(lang: Language) -> &'static LocaleMessages {
    match lang {
        Language::ZhCn => ZH_CN_MESSAGES.get_or_init(|| load_messages(Language::ZhCn)),
        Language::EnUs => EN_US_MESSAGES.get_or_init(|| load_messages(Language::EnUs)),
    }
}

fn reveal_button(messages: &LocaleMessages) -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return messages.reveal_in_file_manager_finder;
    }
    #[cfg(target_os = "windows")]
    {
        return messages.reveal_in_file_manager_explorer;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return messages.reveal_in_file_manager_default;
    }

    #[allow(unreachable_code)]
    messages.reveal_in_file_manager_default
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

    serde_json::from_str::<RawLocaleMessages>(&raw)
        .map(LocaleMessages::from)
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

#[cfg(target_os = "linux")]
fn linux_packaged_i18n_dir(current_exe: &Path) -> Option<PathBuf> {
    let exe_dir = current_exe.parent()?;
    if exe_dir.file_name()?.to_string_lossy() != "bin" {
        return None;
    }
    let prefix_dir = exe_dir.parent()?;
    Some(prefix_dir.join("lib").join("kpdf").join("i18n"))
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
        #[cfg(target_os = "linux")]
        if let Some(packaged_i18n_dir) = linux_packaged_i18n_dir(&current_exe) {
            push_i18n_dir(&mut candidates, &mut seen, packaged_i18n_dir);
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

    #[cfg(target_os = "linux")]
    {
        push_i18n_dir(
            &mut candidates,
            &mut seen,
            PathBuf::from("/usr/lib/kpdf/i18n"),
        );
        push_i18n_dir(
            &mut candidates,
            &mut seen,
            PathBuf::from("/usr/local/lib/kpdf/i18n"),
        );
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

fn format_template(template: &str, vars: &[(&str, String)]) -> String {
    let mut output = template.to_string();
    for (key, value) in vars {
        let token = format!("{{{key}}}");
        output = output.replace(&token, value);
    }
    output
}
