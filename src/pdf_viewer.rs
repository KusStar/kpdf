#[path = "command_panel.rs"]
mod command_panel;
#[path = "display_list.rs"]
mod display_list;
#[cfg(target_os = "macos")]
#[path = "macos_context_menu.rs"]
mod macos_context_menu;
#[path = "menu_bar.rs"]
mod menu_bar;
#[path = "tab.rs"]
pub mod tab;
#[path = "text_selection.rs"]
mod text_selection;
#[path = "thumbnail_list.rs"]
mod thumbnail_list;
#[path = "utils.rs"]
mod utils;

use crate::i18n::{I18n, Language};
use crate::{
    APP_REPOSITORY_URL, CheckForUpdatesMenu, DisableLoggingMenu, EnableLoggingMenu, OpenLogsMenu,
    ShowAboutMenu, ShowSettingsMenu, configure_app_menus, updater,
};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{InputEvent, InputState};
use gpui_component::popover::{Popover, PopoverState};
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use gpui_component::{button::*, *};
#[cfg(target_os = "windows")]
use raw_window_handle::RawWindowHandle;
use std::cell::Cell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use std::time::Duration;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{SW_RESTORE, ShowWindowAsync};

// 定义拖放状态
#[derive(Debug, Clone)]
pub enum DragState {
    None,
    Started {
        source_tab_id: usize,
    },
    Over {
        source_tab_id: usize,
        target_tab_id: usize,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RecentPopupAnchor {
    OpenButton,
    TabAddButton,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct BookmarkEntry {
    path: PathBuf,
    page_index: usize,
    created_at_unix_secs: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum BookmarkScope {
    CurrentPdf,
    All,
}

use self::tab::{PdfTab, TabBar};
use self::text_selection::copy_to_clipboard;
use self::utils::{
    display_file_name, ensure_pdfium_ready, load_display_images, load_document_summary,
};

const ZOOM_MIN: f32 = 0.6;
const ZOOM_MAX: f32 = 1.0;
const ZOOM_STEP: f32 = 0.1;
const SIDEBAR_WIDTH: f32 = 228.0;
const THUMB_MIN_WIDTH: f32 = 96.0;
const THUMB_HORIZONTAL_PADDING: f32 = 16.0;
const THUMB_VERTICAL_PADDING: f32 = 8.0;
const THUMB_BATCH_SIZE: usize = 1;
const THUMB_MAX_PARALLEL_TASKS: usize = 1;
const DISPLAY_MIN_WIDTH: f32 = 220.0;
const DISPLAY_BATCH_SIZE: usize = 1;
const DISPLAY_MAX_PARALLEL_TASKS: usize = 1;
const DISPLAY_SCROLL_SYNC_DELAY_MS: u64 = 140;
const MAX_RECENT_FILES: usize = 12;
const RECENT_FILES_LIST_MAX_HEIGHT: f32 = 280.0;
const RECENT_POPUP_CLOSE_DELAY_MS: u64 = 120;
const BOOKMARK_POPUP_CLOSE_DELAY_MS: u64 = 120;
const RECENT_FILES_TREE: &str = "recent_files";
const FILE_POSITIONS_TREE: &str = "file_positions";
const WINDOW_SIZE_TREE: &str = "window_size";
const OPEN_TABS_TREE: &str = "open_tabs";
const TITLEBAR_PREFERENCES_TREE: &str = "titlebar_preferences";
const BOOKMARKS_TREE: &str = "bookmarks";
const WINDOW_SIZE_KEY_WIDTH: &str = "width";
const WINDOW_SIZE_KEY_HEIGHT: &str = "height";
const OPEN_TABS_KEY_ACTIVE_INDEX: &str = "active_index";
const TITLEBAR_PREFERENCES_KEY_SHOW_NAVIGATION: &str = "show_navigation";
const TITLEBAR_PREFERENCES_KEY_SHOW_ZOOM: &str = "show_zoom";
const TITLE_BAR_HEIGHT: f32 = 34.0;
const TAB_BAR_HEIGHT: f32 = 36.0;
const ABOUT_DIALOG_WIDTH: f32 = 460.0;
const SETTINGS_DIALOG_WIDTH: f32 = 520.0;
#[cfg(target_os = "macos")]
const TITLE_BAR_CONTENT_LEFT_PADDING: f32 = 80.0;
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_CONTENT_LEFT_PADDING: f32 = 12.0;

#[derive(Debug, Clone)]
enum UpdaterUiState {
    Idle,
    Checking,
    UpToDate {
        latest_version: String,
    },
    Available {
        latest_version: String,
        download_url: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy)]
struct TitleBarVisibilityPreferences {
    show_navigation: bool,
    show_zoom: bool,
}

impl Default for TitleBarVisibilityPreferences {
    fn default() -> Self {
        Self {
            show_navigation: true,
            show_zoom: true,
        }
    }
}

pub use self::utils::PageSummary;

#[cfg(target_os = "windows")]
fn restore_window_native(window: &Window) -> bool {
    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return false;
    };

    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return false;
    };

    // raw-window-handle guarantees non-zero HWND for Win32 handles.
    let hwnd = HWND(win32.hwnd.get() as _);
    unsafe { ShowWindowAsync(hwnd, SW_RESTORE).as_bool() }
}

fn zoom_or_restore_window(window: &Window) {
    #[cfg(target_os = "windows")]
    {
        if window.is_maximized() && restore_window_native(window) {
            return;
        }
    }

    window.zoom_window();
}

pub struct PdfViewer {
    focus_handle: FocusHandle,
    language: Language,
    tab_bar: TabBar,
    recent_store: Option<sled::Tree>,
    position_store: Option<sled::Tree>,
    window_size_store: Option<sled::Tree>,
    open_tabs_store: Option<sled::Tree>,
    titlebar_preferences_store: Option<sled::Tree>,
    bookmarks_store: Option<sled::Tree>,
    last_window_size: Option<(f32, f32)>,
    titlebar_preferences: TitleBarVisibilityPreferences,
    recent_files: Vec<PathBuf>,
    recent_popup_open: bool,
    recent_popup_trigger_hovered: bool,
    recent_popup_tab_trigger_hovered: bool,
    recent_popup_panel_hovered: bool,
    recent_popup_hover_epoch: u64,
    recent_popup_anchor: Option<RecentPopupAnchor>,
    bookmarks: Vec<BookmarkEntry>,
    bookmark_popup_open: bool,
    bookmark_scope: BookmarkScope,
    bookmark_popup_trigger_hovered: bool,
    bookmark_popup_panel_hovered: bool,
    bookmark_popup_hover_epoch: u64,
    about_dialog_open: bool,
    settings_dialog_open: bool,
    updater_state: UpdaterUiState,
    command_panel_open: bool,
    command_panel_query: String,
    command_panel_selected_index: usize,
    tab_bar_scroll: ScrollHandle,
    recent_popup_list_scroll: ScrollHandle,
    bookmark_popup_list_scroll: ScrollHandle,
    command_panel_list_scroll: ScrollHandle,
    recent_home_list_scroll: ScrollHandle,
    command_panel_input_state: Entity<InputState>,
    _command_panel_input_subscription: Subscription,
    context_menu_open: bool,
    context_menu_position: Option<Point<Pixels>>,
    context_menu_tab_id: Option<usize>,
    hovered_tab_id: Option<usize>,
    // 拖放相关状态
    drag_state: DragState,
    drag_mouse_position: Option<Point<Pixels>>,
    text_hover_target: Option<(usize, usize)>, // (tab_id, page_index)
    needs_initial_focus: bool,
    command_panel_needs_focus: bool,
    needs_root_refocus: bool,
    resize_restore_epoch: u64,
}

impl PdfViewer {
    fn now_unix_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language = Language::detect();
        let (
            recent_store,
            position_store,
            window_size_store,
            open_tabs_store,
            titlebar_preferences_store,
            bookmarks_store,
        ) = Self::open_persistent_stores();
        let recent_files = recent_store
            .as_ref()
            .map(Self::load_recent_files_from_store)
            .unwrap_or_default();
        let (saved_open_tab_paths, saved_active_open_tab_index) = open_tabs_store
            .as_ref()
            .map(Self::load_open_tabs_from_store)
            .unwrap_or_else(|| (Vec::new(), None));
        let titlebar_preferences = titlebar_preferences_store
            .as_ref()
            .map(Self::load_titlebar_preferences_from_store)
            .unwrap_or_default();
        let bookmarks = bookmarks_store
            .as_ref()
            .map(Self::load_bookmarks_from_store)
            .unwrap_or_default();
        let command_panel_input_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(I18n::new(language).command_panel_search_hint())
        });
        let command_panel_input_state_for_sub = command_panel_input_state.clone();
        let command_panel_input_subscription = cx.subscribe(
            &command_panel_input_state_for_sub,
            |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let next_query = input.read(cx).value().to_string();
                if this.command_panel_query != next_query {
                    this.command_panel_query = next_query;
                    this.command_panel_selected_index = 0;
                    this.command_panel_list_scroll.scroll_to_item(0);
                    if this.command_panel_open {
                        cx.notify();
                    }
                }
            },
        );

        let mut tab_bar = TabBar::new();
        let mut tabs_to_restore = Vec::new();
        for path in saved_open_tab_paths {
            if !path.exists() {
                continue;
            }
            let tab_id = tab_bar.create_tab_with_path(path.clone(), Vec::new());
            tabs_to_restore.push((tab_id, path));
        }

        if tabs_to_restore.is_empty() {
            // 没有可恢复标签时，创建第一个空标签页
            tab_bar.create_tab();
        } else {
            let target_active_index = saved_active_open_tab_index
                .unwrap_or_else(|| tabs_to_restore.len().saturating_sub(1))
                .min(tabs_to_restore.len().saturating_sub(1));
            if let Some((tab_id, _)) = tabs_to_restore.get(target_active_index) {
                tab_bar.switch_to_tab(*tab_id);
            }
        }

        let mut viewer = Self {
            focus_handle: cx.focus_handle(),
            language,
            tab_bar,
            recent_store,
            position_store,
            window_size_store,
            open_tabs_store,
            titlebar_preferences_store,
            bookmarks_store,
            last_window_size: None,
            titlebar_preferences,
            recent_files,
            recent_popup_open: false,
            recent_popup_trigger_hovered: false,
            recent_popup_tab_trigger_hovered: false,
            recent_popup_panel_hovered: false,
            recent_popup_hover_epoch: 0,
            recent_popup_anchor: None,
            bookmarks,
            bookmark_popup_open: false,
            bookmark_scope: BookmarkScope::CurrentPdf,
            bookmark_popup_trigger_hovered: false,
            bookmark_popup_panel_hovered: false,
            bookmark_popup_hover_epoch: 0,
            about_dialog_open: false,
            settings_dialog_open: false,
            updater_state: UpdaterUiState::Idle,
            command_panel_open: false,
            command_panel_query: String::new(),
            command_panel_selected_index: 0,
            tab_bar_scroll: ScrollHandle::new(),
            recent_popup_list_scroll: ScrollHandle::new(),
            bookmark_popup_list_scroll: ScrollHandle::new(),
            command_panel_list_scroll: ScrollHandle::new(),
            recent_home_list_scroll: ScrollHandle::new(),
            command_panel_input_state,
            _command_panel_input_subscription: command_panel_input_subscription,
            context_menu_open: false,
            context_menu_position: None,
            context_menu_tab_id: None,
            hovered_tab_id: None,
            drag_state: DragState::None,
            drag_mouse_position: None,
            text_hover_target: None,
            needs_initial_focus: true,
            command_panel_needs_focus: false,
            needs_root_refocus: false,
            resize_restore_epoch: 0,
        };

        viewer.persist_open_tabs();
        if !tabs_to_restore.is_empty()
            && let Err(err) = ensure_pdfium_ready(language)
        {
            crate::debug_log!("[pdfium] pre-init before restoring tabs failed: {}", err);
        }
        viewer.restore_open_tabs(tabs_to_restore, cx);
        viewer
    }

    fn i18n(&self) -> I18n {
        I18n::new(self.language)
    }

    fn active_tab(&self) -> Option<&PdfTab> {
        self.tab_bar.get_active_tab()
    }

    fn active_tab_mut(&mut self) -> Option<&mut PdfTab> {
        self.tab_bar.get_active_tab_mut()
    }

    #[allow(dead_code)]
    fn with_active_tab<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&PdfTab) -> R,
    {
        self.active_tab().map(f)
    }

    #[allow(dead_code)]
    fn with_active_tab_mut<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut PdfTab) -> R,
    {
        self.active_tab_mut().map(f)
    }

    fn open_persistent_stores() -> (
        Option<sled::Tree>,
        Option<sled::Tree>,
        Option<sled::Tree>,
        Option<sled::Tree>,
        Option<sled::Tree>,
        Option<sled::Tree>,
    ) {
        let db_path = Self::recent_files_db_path();
        if let Some(parent) = db_path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                crate::debug_log!("[store] create dir failed: {}", parent.to_string_lossy());
                return (None, None, None, None, None, None);
            }
        }

        let db = match sled::open(&db_path) {
            Ok(db) => db,
            Err(err) => {
                crate::debug_log!(
                    "[store] open db failed: {} | {}",
                    db_path.to_string_lossy(),
                    err
                );
                return (None, None, None, None, None, None);
            }
        };

        let recent_store = match db.open_tree(RECENT_FILES_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!("[store] open tree failed: {} | {}", RECENT_FILES_TREE, err);
                None
            }
        };
        let position_store = match db.open_tree(FILE_POSITIONS_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!(
                    "[store] open tree failed: {} | {}",
                    FILE_POSITIONS_TREE,
                    err
                );
                None
            }
        };
        let window_size_store = match db.open_tree(WINDOW_SIZE_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!("[store] open tree failed: {} | {}", WINDOW_SIZE_TREE, err);
                None
            }
        };
        let open_tabs_store = match db.open_tree(OPEN_TABS_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!("[store] open tree failed: {} | {}", OPEN_TABS_TREE, err);
                None
            }
        };
        let titlebar_preferences_store = match db.open_tree(TITLEBAR_PREFERENCES_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!(
                    "[store] open tree failed: {} | {}",
                    TITLEBAR_PREFERENCES_TREE,
                    err
                );
                None
            }
        };
        let bookmarks_store = match db.open_tree(BOOKMARKS_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!("[store] open tree failed: {} | {}", BOOKMARKS_TREE, err);
                None
            }
        };

        crate::debug_log!(
            "[store] init recent={} positions={} window_size={} open_tabs={} titlebar_preferences={} bookmarks={} path={}",
            recent_store.is_some(),
            position_store.is_some(),
            window_size_store.is_some(),
            open_tabs_store.is_some(),
            titlebar_preferences_store.is_some(),
            bookmarks_store.is_some(),
            db_path.to_string_lossy()
        );

        (
            recent_store,
            position_store,
            window_size_store,
            open_tabs_store,
            titlebar_preferences_store,
            bookmarks_store,
        )
    }

    fn recent_files_db_path() -> PathBuf {
        if let Some(app_data) = std::env::var_os("APPDATA") {
            return PathBuf::from(app_data).join("kpdf").join("recent_files_db");
        }

        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".kpdf").join("recent_files_db");
        }

        PathBuf::from(".kpdf").join("recent_files_db")
    }

    fn load_recent_files_from_store(store: &sled::Tree) -> Vec<PathBuf> {
        store
            .iter()
            .filter_map(|entry| {
                let (_, value) = entry.ok()?;
                let path_str = String::from_utf8(value.to_vec()).ok()?;
                if path_str.is_empty() {
                    return None;
                }
                Some(PathBuf::from(path_str))
            })
            .take(MAX_RECENT_FILES)
            .collect()
    }

    fn decode_bookmark_entry_from_store(value: &[u8]) -> Option<BookmarkEntry> {
        if value.len() < 9 {
            return None;
        }

        let mut page_bytes = [0u8; 8];
        page_bytes.copy_from_slice(&value[0..8]);
        let page_index = usize::try_from(u64::from_be_bytes(page_bytes)).ok()?;
        let (created_at_unix_secs, path_bytes) = if value.len() >= 17 {
            let mut created_at_bytes = [0u8; 8];
            created_at_bytes.copy_from_slice(&value[8..16]);
            (u64::from_be_bytes(created_at_bytes), &value[16..])
        } else {
            // Backward compatibility with older layout: [8-byte page][path bytes]
            (0, &value[8..])
        };

        let path_str = String::from_utf8(path_bytes.to_vec()).ok()?;
        if path_str.is_empty() {
            return None;
        }

        Some(BookmarkEntry {
            path: PathBuf::from(path_str),
            page_index,
            created_at_unix_secs,
        })
    }

    fn load_bookmarks_from_store(store: &sled::Tree) -> Vec<BookmarkEntry> {
        let mut indexed_bookmarks = Vec::new();
        for entry in store.iter() {
            let (key, value) = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if key.len() != 4 {
                continue;
            }
            let bookmark_index = u32::from_be_bytes([key[0], key[1], key[2], key[3]]) as usize;
            let Some(bookmark) = Self::decode_bookmark_entry_from_store(value.as_ref()) else {
                continue;
            };
            indexed_bookmarks.push((bookmark_index, bookmark));
        }
        indexed_bookmarks.sort_by_key(|(index, _)| *index);
        indexed_bookmarks
            .into_iter()
            .map(|(_, bookmark)| bookmark)
            .collect()
    }

    fn load_open_tabs_from_store(store: &sled::Tree) -> (Vec<PathBuf>, Option<usize>) {
        let mut indexed_tabs = Vec::new();
        for entry in store.iter() {
            let (key, value) = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if key.len() != 4 {
                continue;
            }
            let tab_index = u32::from_be_bytes([key[0], key[1], key[2], key[3]]) as usize;
            let path_str = match String::from_utf8(value.to_vec()) {
                Ok(path) if !path.is_empty() => path,
                _ => continue,
            };
            indexed_tabs.push((tab_index, PathBuf::from(path_str)));
        }
        indexed_tabs.sort_by_key(|(index, _)| *index);

        let active_index = store
            .get(OPEN_TABS_KEY_ACTIVE_INDEX)
            .ok()
            .flatten()
            .and_then(|raw| {
                if raw.len() != 8 {
                    return None;
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(raw.as_ref());
                usize::try_from(u64::from_be_bytes(bytes)).ok()
            });

        (
            indexed_tabs
                .into_iter()
                .map(|(_, path)| path)
                .collect::<Vec<_>>(),
            active_index,
        )
    }

    fn decode_stored_bool(value: Option<sled::IVec>, default: bool) -> bool {
        let Some(raw) = value else {
            return default;
        };
        raw.first().copied().map(|v| v != 0).unwrap_or(default)
    }

    fn load_titlebar_preferences_from_store(store: &sled::Tree) -> TitleBarVisibilityPreferences {
        let default = TitleBarVisibilityPreferences::default();
        TitleBarVisibilityPreferences {
            show_navigation: Self::decode_stored_bool(
                store
                    .get(TITLEBAR_PREFERENCES_KEY_SHOW_NAVIGATION)
                    .ok()
                    .flatten(),
                default.show_navigation,
            ),
            show_zoom: Self::decode_stored_bool(
                store.get(TITLEBAR_PREFERENCES_KEY_SHOW_ZOOM).ok().flatten(),
                default.show_zoom,
            ),
        }
    }

    fn persist_titlebar_preferences(&self) {
        let Some(store) = self.titlebar_preferences_store.as_ref() else {
            return;
        };

        if store
            .insert(
                TITLEBAR_PREFERENCES_KEY_SHOW_NAVIGATION,
                [u8::from(self.titlebar_preferences.show_navigation)].as_slice(),
            )
            .is_err()
        {
            return;
        }
        if store
            .insert(
                TITLEBAR_PREFERENCES_KEY_SHOW_ZOOM,
                [u8::from(self.titlebar_preferences.show_zoom)].as_slice(),
            )
            .is_err()
        {
            return;
        }

        let _ = store.flush();
    }

    fn persist_open_tabs(&self) {
        let Some(store) = self.open_tabs_store.as_ref() else {
            return;
        };

        if store.clear().is_err() {
            return;
        }

        let active_tab_id = self.tab_bar.active_tab_id();
        let mut active_index = None;
        let mut open_paths = Vec::new();
        for tab in self.tab_bar.tabs() {
            let Some(path) = tab.path.as_ref() else {
                continue;
            };
            if active_tab_id == Some(tab.id) {
                active_index = Some(open_paths.len());
            }
            open_paths.push(path.clone());
        }

        for (index, path) in open_paths.iter().enumerate() {
            let key = (index as u32).to_be_bytes();
            if store
                .insert(key, path.to_string_lossy().as_bytes())
                .is_err()
            {
                return;
            }
        }

        if let Some(index) = active_index {
            let active_bytes = (index as u64).to_be_bytes();
            if store
                .insert(OPEN_TABS_KEY_ACTIVE_INDEX, active_bytes.as_slice())
                .is_err()
            {
                return;
            }
        }

        let _ = store.flush();
    }

    fn persist_bookmarks(&self) {
        let Some(store) = self.bookmarks_store.as_ref() else {
            return;
        };

        if store.clear().is_err() {
            return;
        }

        for (index, bookmark) in self.bookmarks.iter().enumerate() {
            let key = (index as u32).to_be_bytes();
            let page_index = (bookmark.page_index as u64).to_be_bytes();
            let created_at = bookmark.created_at_unix_secs.to_be_bytes();
            let path = bookmark.path.to_string_lossy();

            let mut value = Vec::with_capacity(16 + path.len());
            value.extend_from_slice(&page_index);
            value.extend_from_slice(&created_at);
            value.extend_from_slice(path.as_bytes());

            if store.insert(key, value).is_err() {
                return;
            }
        }

        let _ = store.flush();
    }

    fn restore_open_tabs(
        &mut self,
        tabs_to_restore: Vec<(usize, PathBuf)>,
        cx: &mut Context<Self>,
    ) {
        let active_tab_id = self.tab_bar.active_tab_id();
        for (tab_id, path) in tabs_to_restore {
            if Some(tab_id) == active_tab_id {
                self.load_pdf_path_into_tab(tab_id, path, false, cx);
                break;
            }
        }
    }

    fn pending_load_path_for_tab(&self, tab_id: usize) -> Option<PathBuf> {
        self.tab_bar
            .tabs()
            .iter()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| {
                if tab.summary_loading {
                    return None;
                }
                if tab.summary_failed || !tab.summary_loaded {
                    return tab.path.clone();
                }
                None
            })
    }

    fn load_tab_if_needed(&mut self, tab_id: usize, cx: &mut Context<Self>) -> bool {
        if let Some(path) = self.pending_load_path_for_tab(tab_id) {
            self.load_pdf_path_into_tab(tab_id, path, false, cx);
            return true;
        }
        false
    }

    fn load_pdf_path_into_tab(
        &mut self,
        tab_id: usize,
        path: PathBuf,
        remember_recent_file: bool,
        cx: &mut Context<Self>,
    ) {
        let language = self.language;

        if let Some(tab) = self.tab_bar.get_tab_mut(tab_id) {
            tab.path = Some(path.clone());
            tab.pages.clear();
            tab.summary_loaded = false;
            tab.summary_loading = true;
            tab.summary_failed = false;
            tab.selected_page = 0;
            tab.active_page = 0;
            tab.zoom = 1.0;
            tab.last_saved_position = None;
            tab.reset_page_render_state();
        } else {
            return;
        }

        self.persist_open_tabs();
        if self.tab_bar.active_tab_id() == Some(tab_id) {
            self.scroll_tab_bar_to_active_tab();
        }
        cx.notify();

        cx.spawn(async move |view, cx| {
            let parsed = cx
                .background_executor()
                .spawn({
                    let path = path.clone();
                    async move { load_document_summary(&path, language) }
                })
                .await;

            let _ = view.update(cx, |this, cx| {
                let restored_page = this.load_saved_file_position(&path);
                let mut loaded_ok = false;

                if let Some(tab) = this.tab_bar.get_tab_mut(tab_id) {
                    if tab.path.as_ref() != Some(&path) {
                        return;
                    }
                    tab.path = Some(path.clone());
                    match parsed {
                        Ok(mut pages) => {
                            pages.sort_by_key(|p| p.index);
                            tab.pages = pages;
                            tab.summary_loaded = true;
                            tab.summary_loading = false;
                            tab.summary_failed = false;

                            let initial_page = restored_page
                                .unwrap_or(0)
                                .min(tab.pages.len().saturating_sub(1));
                            tab.selected_page = initial_page;
                            tab.active_page = initial_page;
                            tab.zoom = 1.0;
                            tab.reset_page_render_state();

                            if !tab.pages.is_empty() {
                                let strategy = if initial_page == 0 {
                                    ScrollStrategy::Top
                                } else {
                                    ScrollStrategy::Center
                                };
                                tab.suppress_display_scroll_sync_once = true;
                                tab.thumbnail_scroll.scroll_to_item(initial_page, strategy);
                                tab.display_scroll
                                    .scroll_to_item(initial_page, ScrollStrategy::Top);
                            }
                            loaded_ok = true;
                        }
                        Err(_) => {
                            tab.pages.clear();
                            tab.summary_loaded = false;
                            tab.summary_loading = false;
                            tab.summary_failed = true;
                            tab.selected_page = 0;
                            tab.active_page = 0;
                            tab.zoom = 1.0;
                            tab.reset_page_render_state();
                        }
                    }
                }

                if loaded_ok && remember_recent_file {
                    this.remember_recent_file(&path);
                }

                this.persist_open_tabs();
                if this.tab_bar.active_tab_id() == Some(tab_id) {
                    this.scroll_tab_bar_to_active_tab();
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_pdf_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.close_command_panel(cx);
        self.close_recent_popup(cx);
        self.close_bookmark_popup(cx);

        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(self.i18n().open_pdf_prompt().into()),
        });

        cx.spawn(async move |view, cx| {
            let picker_result = picker.await;
            match picker_result {
                Ok(Ok(Some(paths))) => {
                    for (i, path) in paths.into_iter().enumerate() {
                        let is_first = i == 0;
                        let _ = view.update(cx, |this, cx| {
                            if is_first
                                && this.active_tab().map(|t| t.path.is_none()).unwrap_or(false)
                            {
                                // 第一个文件在当前标签页打开
                                this.open_pdf_path_in_current_tab(path, cx);
                            } else {
                                // 其他文件在新标签页打开
                                this.open_pdf_path_in_new_tab(path, cx);
                            }
                        });
                    }
                }
                _ => {}
            }
        })
        .detach();
    }

    fn open_recent_pdf(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.exists() {
            self.recent_files.retain(|p| p != &path);
            self.persist_recent_files();
            cx.notify();
            return;
        }

        // 检查是否已经在某个标签页打开
        for tab in self.tab_bar.tabs() {
            if tab.path.as_ref() == Some(&path) {
                // 切换到已打开的标签页
                self.switch_to_tab(tab.id, cx);
                return;
            }
        }

        // 在新标签页打开
        self.open_pdf_path_in_new_tab(path, cx);
    }

    fn open_pdf_path_in_current_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let tab_id = self
            .tab_bar
            .active_tab_id()
            .unwrap_or_else(|| self.tab_bar.create_tab());
        let _ = self.tab_bar.switch_to_tab(tab_id);
        self.load_pdf_path_into_tab(tab_id, path, true, cx);
    }

    fn open_pdf_path_in_new_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let tab_id = self.tab_bar.create_tab();
        let _ = self.tab_bar.switch_to_tab(tab_id);
        self.load_pdf_path_into_tab(tab_id, path, true, cx);
    }

    fn reveal_path_in_file_manager(&self, path: &Path) {
        let status = {
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open")
                    .arg("-R")
                    .arg(path)
                    .status()
            }
            #[cfg(target_os = "windows")]
            {
                let select_arg = format!("/select,{}", path.to_string_lossy());
                std::process::Command::new("explorer")
                    .arg(select_arg)
                    .status()
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                let open_target = if path.is_dir() {
                    path.to_path_buf()
                } else {
                    path.parent()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| path.to_path_buf())
                };
                std::process::Command::new("xdg-open")
                    .arg(&open_target)
                    .status()
            }
        };

        match status {
            Ok(exit_status) if exit_status.success() => {
                crate::debug_log!("[tab] revealed in file manager: {}", path.display());
            }
            Ok(exit_status) => {
                crate::debug_log!(
                    "[tab] failed to reveal in file manager: {} | exit={}",
                    path.display(),
                    exit_status
                );
            }
            Err(err) => {
                crate::debug_log!(
                    "[tab] failed to reveal in file manager: {} | {}",
                    path.display(),
                    err
                );
            }
        }
    }

    fn reveal_tab_in_file_manager(&self, tab_id: usize) {
        let tab_path = self
            .tab_bar
            .tabs()
            .iter()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.path.as_ref());
        let Some(path) = tab_path else {
            crate::debug_log!("[tab] cannot reveal tab {}: no file path", tab_id);
            return;
        };

        self.reveal_path_in_file_manager(path);
    }

    fn open_logs_directory(&self) {
        let Some(log_file_path) = crate::logger::log_file_path() else {
            crate::debug_log!("[log] cannot open logs directory: unresolved log path");
            return;
        };

        let log_dir = log_file_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or(log_file_path);

        let status = {
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open").arg(&log_dir).status()
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("explorer")
                    .arg(&log_dir)
                    .status()
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                std::process::Command::new("xdg-open")
                    .arg(&log_dir)
                    .status()
            }
        };

        match status {
            Ok(exit_status) if exit_status.success() => {
                crate::debug_log!("[log] opened logs directory: {}", log_dir.display());
            }
            Ok(exit_status) => {
                crate::debug_log!(
                    "[log] failed to open logs directory: {} | exit={}",
                    log_dir.display(),
                    exit_status
                );
            }
            Err(err) => {
                crate::debug_log!(
                    "[log] failed to open logs directory: {} | {}",
                    log_dir.display(),
                    err
                );
            }
        }
    }

    fn summarize_updater_error(raw: &str) -> String {
        const MAX_LEN: usize = 88;
        let mut message = raw
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or(raw)
            .trim()
            .to_string();
        if message.is_empty() {
            message = "unknown error".to_string();
        }
        if message.len() > MAX_LEN {
            message.truncate(MAX_LEN - 3);
            message.push_str("...");
        }
        message
    }

    fn updater_status_text(&self) -> String {
        let i18n = self.i18n();
        match &self.updater_state {
            UpdaterUiState::Idle => i18n.update_status_idle().to_string(),
            UpdaterUiState::Checking => i18n.update_status_checking().to_string(),
            UpdaterUiState::UpToDate { latest_version } => {
                i18n.update_status_up_to_date(latest_version)
            }
            UpdaterUiState::Available { latest_version, .. } => {
                i18n.update_status_available(latest_version)
            }
            UpdaterUiState::Error { message } => i18n.update_status_failed(message),
        }
    }

    fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if matches!(self.updater_state, UpdaterUiState::Checking) {
            return;
        }

        self.updater_state = UpdaterUiState::Checking;
        cx.notify();

        cx.spawn(async move |view, cx| {
            let update_result = cx
                .background_executor()
                .spawn(async move { updater::check_for_updates(env!("CARGO_PKG_VERSION")) })
                .await;

            let _ = view.update(cx, |this, cx| {
                match update_result {
                    Ok(updater::UpdateCheck::UpToDate { latest_version }) => {
                        this.updater_state = UpdaterUiState::UpToDate { latest_version };
                    }
                    Ok(updater::UpdateCheck::UpdateAvailable(info)) => {
                        this.updater_state = UpdaterUiState::Available {
                            latest_version: info.latest_version,
                            download_url: info.download_url,
                        };
                    }
                    Err(err) => {
                        crate::debug_log!("[updater] check failed: {}", err);
                        this.updater_state = UpdaterUiState::Error {
                            message: Self::summarize_updater_error(&err.to_string()),
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_settings_dialog(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        if self.command_panel_open {
            self.close_command_panel(cx);
            changed = true;
        }
        if self.recent_popup_open {
            self.close_recent_popup(cx);
            changed = true;
        }
        if self.bookmark_popup_open {
            self.close_bookmark_popup(cx);
            changed = true;
        }
        if self.about_dialog_open {
            self.about_dialog_open = false;
            changed = true;
        }
        if !self.settings_dialog_open {
            self.settings_dialog_open = true;
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }

    fn close_settings_dialog(&mut self, cx: &mut Context<Self>) {
        if self.settings_dialog_open {
            self.settings_dialog_open = false;
            self.needs_root_refocus = true;
            cx.notify();
        }
    }

    fn set_titlebar_navigation_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.titlebar_preferences.show_navigation == visible {
            return;
        }
        self.titlebar_preferences.show_navigation = visible;
        self.persist_titlebar_preferences();
        cx.notify();
    }

    fn set_titlebar_zoom_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.titlebar_preferences.show_zoom == visible {
            return;
        }
        self.titlebar_preferences.show_zoom = visible;
        self.persist_titlebar_preferences();
        cx.notify();
    }

    fn render_settings_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.settings_dialog_open {
            return None;
        }

        let i18n = self.i18n();

        Some(
            div()
                .id("settings-dialog-overlay")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .bg(cx.theme().background.opacity(0.45))
                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                    cx.stop_propagation();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.close_settings_dialog(cx);
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .v_flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("settings-dialog")
                                .w(px(SETTINGS_DIALOG_WIDTH))
                                .v_flex()
                                .gap_3()
                                .popover_style(cx)
                                .p_4()
                                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                                    cx.stop_propagation();
                                }))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_, _, _, cx| {
                                        cx.stop_propagation();
                                    }),
                                )
                                .child(
                                    div()
                                        .text_lg()
                                        .text_color(cx.theme().foreground)
                                        .child(i18n.settings_dialog_title()),
                                )
                                .child(div().h(px(1.)).bg(cx.theme().border))
                                .child(
                                    div()
                                        .v_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(i18n.settings_titlebar_section()),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .rounded_md()
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .p_3()
                                                .v_flex()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .flex()
                                                        .items_start()
                                                        .justify_between()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .v_flex()
                                                                .items_start()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(cx.theme().foreground)
                                                                        .child(
                                                                            i18n.settings_titlebar_navigation_label(),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .whitespace_normal()
                                                                        .child(
                                                                            i18n.settings_titlebar_navigation_hint(),
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            Checkbox::new("settings-show-titlebar-navigation")
                                                                .checked(
                                                                    self.titlebar_preferences
                                                                        .show_navigation,
                                                                )
                                                                .on_click(cx.listener(
                                                                    |this, checked: &bool, _, cx| {
                                                                        this.set_titlebar_navigation_visible(
                                                                            *checked,
                                                                            cx,
                                                                        );
                                                                    },
                                                                )),
                                                        ),
                                                )
                                                .child(div().h(px(1.)).bg(cx.theme().border))
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .flex()
                                                        .items_start()
                                                        .justify_between()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .v_flex()
                                                                .items_start()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(cx.theme().foreground)
                                                                        .child(
                                                                            i18n.settings_titlebar_zoom_label(),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .whitespace_normal()
                                                                        .child(
                                                                            i18n.settings_titlebar_zoom_hint(),
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            Checkbox::new("settings-show-titlebar-zoom")
                                                                .checked(self.titlebar_preferences.show_zoom)
                                                                .on_click(cx.listener(
                                                                    |this, checked: &bool, _, cx| {
                                                                        this.set_titlebar_zoom_visible(
                                                                            *checked,
                                                                            cx,
                                                                        );
                                                                    },
                                                                )),
                                                        ),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .child(
                                            Button::new("settings-close")
                                                .small()
                                                .ghost()
                                                .label(i18n.close_button())
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.close_settings_dialog(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    fn open_about_dialog(&mut self, cx: &mut Context<Self>) {
        if self.command_panel_open {
            self.close_command_panel(cx);
        }
        if self.recent_popup_open {
            self.close_recent_popup(cx);
        }
        if self.bookmark_popup_open {
            self.close_bookmark_popup(cx);
        }
        if self.settings_dialog_open {
            self.settings_dialog_open = false;
        }
        if !self.about_dialog_open {
            self.about_dialog_open = true;
            cx.notify();
        }
    }

    fn close_about_dialog(&mut self, cx: &mut Context<Self>) {
        if self.about_dialog_open {
            self.about_dialog_open = false;
            self.needs_root_refocus = true;
            cx.notify();
        }
    }

    fn render_about_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.about_dialog_open {
            return None;
        }

        let i18n = self.i18n();
        let version = env!("CARGO_PKG_VERSION");
        let updater_status = self.updater_status_text();
        let updater_download_url = match &self.updater_state {
            UpdaterUiState::Available { download_url, .. } => Some(download_url.clone()),
            _ => None,
        };
        let updater_is_checking = matches!(self.updater_state, UpdaterUiState::Checking);

        Some(
            div()
                .id("about-dialog-overlay")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .bg(cx.theme().background.opacity(0.45))
                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                    cx.stop_propagation();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.close_about_dialog(cx);
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .v_flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("about-dialog")
                                .w(px(ABOUT_DIALOG_WIDTH))
                                .v_flex()
                                .gap_3()
                                .popover_style(cx)
                                .p_4()
                                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                                    cx.stop_propagation();
                                }))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_, _, _, cx| {
                                        cx.stop_propagation();
                                    }),
                                )
                                .child(
                                    div()
                                        .v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_lg()
                                                .text_color(cx.theme().foreground)
                                                .child(format!(
                                                    "{} kPDF",
                                                    i18n.about_dialog_title()
                                                )),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(i18n.about_app_info()),
                                        ),
                                )
                                .child(div().h(px(1.)).bg(cx.theme().border))
                                .child(
                                    div()
                                        .v_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(i18n.version_label()),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().foreground)
                                                        .child(version),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .v_flex()
                                                .items_start()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(i18n.website_label()),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().foreground)
                                                        .child(APP_REPOSITORY_URL),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .v_flex()
                                                .items_start()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(i18n.updates_label()),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().foreground)
                                                        .whitespace_normal()
                                                        .child(updater_status),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("about-open-website")
                                                .small()
                                                .label(i18n.open_website_button())
                                                .on_click(|_, _, cx| {
                                                    cx.open_url(APP_REPOSITORY_URL);
                                                }),
                                        )
                                        .child(
                                            Button::new("about-check-updates")
                                                .small()
                                                .ghost()
                                                .label(i18n.check_updates_button())
                                                .disabled(updater_is_checking)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.check_for_updates(cx);
                                                })),
                                        )
                                        .when_some(updater_download_url, |this, download_url| {
                                            this.child(
                                                Button::new("about-download-update")
                                                    .small()
                                                    .label(i18n.download_update_button())
                                                    .on_click(move |_, _, cx| {
                                                        cx.open_url(download_url.as_str());
                                                    }),
                                            )
                                        })
                                        .child(
                                            Button::new("about-close")
                                                .small()
                                                .ghost()
                                                .label(i18n.close_button())
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.close_about_dialog(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    #[allow(dead_code)]
    fn create_new_tab(&mut self, cx: &mut Context<Self>) {
        self.tab_bar.create_tab();
        self.persist_open_tabs();
        cx.notify();
    }

    fn save_tab_position_if_needed(&self, tab_id: usize) {
        if let Some(tab) = self.tab_bar.tabs().iter().find(|t| t.id == tab_id)
            && let Some(path) = tab.path.as_ref()
            && !tab.pages.is_empty()
        {
            let page_index = tab.active_page.min(tab.pages.len().saturating_sub(1));
            self.save_file_position(path, page_index);
        }
    }

    fn close_tabs_by_ids(&mut self, tab_ids: Vec<usize>, cx: &mut Context<Self>) {
        if tab_ids.is_empty() {
            return;
        }

        for tab_id in &tab_ids {
            self.save_tab_position_if_needed(*tab_id);
        }

        for tab_id in tab_ids {
            self.tab_bar.close_tab(tab_id);
        }

        // 如果没有标签页了，创建一个空的
        if !self.tab_bar.has_tabs() {
            self.tab_bar.create_tab();
        }

        self.persist_open_tabs();
        self.scroll_tab_bar_to_active_tab();
        if let Some(active_tab_id) = self.tab_bar.active_tab_id()
            && self.load_tab_if_needed(active_tab_id, cx)
        {
            return;
        }
        cx.notify();
    }

    fn close_all_tabs(&mut self, cx: &mut Context<Self>) {
        let tab_ids = self.tab_bar.tabs().iter().map(|tab| tab.id).collect();
        self.close_tabs_by_ids(tab_ids, cx);
    }

    fn close_other_tabs(&mut self, keep_tab_id: usize, cx: &mut Context<Self>) {
        if self.tab_bar.get_tab_index_by_id(keep_tab_id).is_none() {
            return;
        }

        let _ = self.tab_bar.switch_to_tab(keep_tab_id);
        let tab_ids = self
            .tab_bar
            .tabs()
            .iter()
            .filter_map(|tab| (tab.id != keep_tab_id).then_some(tab.id))
            .collect::<Vec<_>>();
        if tab_ids.is_empty() {
            self.persist_open_tabs();
            self.scroll_tab_bar_to_active_tab();
            cx.notify();
            return;
        }

        self.close_tabs_by_ids(tab_ids, cx);
    }

    fn close_tab(&mut self, tab_id: usize, cx: &mut Context<Self>) {
        self.close_tabs_by_ids(vec![tab_id], cx);
    }

    fn switch_to_tab(&mut self, tab_id: usize, cx: &mut Context<Self>) {
        if self.tab_bar.switch_to_tab(tab_id) {
            self.persist_open_tabs();
            self.scroll_tab_bar_to_active_tab();
            if self.load_tab_if_needed(tab_id, cx) {
                return;
            }
            cx.notify();
        }
    }

    fn visible_tab_ids(&self) -> Vec<usize> {
        let tabs = self.tab_bar.tabs();
        let has_file_open = tabs.iter().any(|tab| tab.path.is_some());
        tabs.iter()
            .filter(|tab| !has_file_open || tab.path.is_some())
            .map(|tab| tab.id)
            .collect()
    }

    fn switch_to_visible_tab_by_index(&mut self, visible_index: usize, cx: &mut Context<Self>) {
        let visible_tabs = self.visible_tab_ids();
        if visible_tabs.is_empty() {
            return;
        }
        let target_index = visible_index.min(visible_tabs.len().saturating_sub(1));
        self.switch_to_tab(visible_tabs[target_index], cx);
    }

    fn switch_visible_tab_by_offset(&mut self, offset: isize, cx: &mut Context<Self>) {
        let visible_tabs = self.visible_tab_ids();
        if visible_tabs.len() < 2 {
            return;
        }

        let current_index = self
            .tab_bar
            .active_tab_id()
            .and_then(|id| visible_tabs.iter().position(|tab_id| *tab_id == id))
            .unwrap_or(0);
        let len = visible_tabs.len() as isize;
        let next_index = (current_index as isize + offset).rem_euclid(len) as usize;
        self.switch_to_tab(visible_tabs[next_index], cx);
    }

    fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_primary_modifier = event.keystroke.modifiers.secondary();
        let key = event.keystroke.key.as_str();

        if self.settings_dialog_open {
            if key == "escape" {
                self.close_settings_dialog(cx);
                cx.stop_propagation();
            }
            return;
        }

        if self.about_dialog_open {
            if key == "escape" {
                self.close_about_dialog(cx);
                cx.stop_propagation();
            }
            return;
        }

        if self.command_panel_open {
            if key == "escape" {
                self.close_command_panel(cx);
                cx.stop_propagation();
                return;
            }
            if key == "down" {
                self.move_command_panel_selection(1, cx);
                cx.stop_propagation();
                return;
            }
            if key == "up" {
                self.move_command_panel_selection(-1, cx);
                cx.stop_propagation();
                return;
            }
            if key == "enter" {
                self.execute_command_panel_selected(window, cx);
                cx.stop_propagation();
                return;
            }
            if key == "t" && is_primary_modifier && !event.keystroke.modifiers.shift {
                self.toggle_command_panel(window, cx);
                cx.stop_propagation();
                return;
            }
            // Keep command panel focused on query editing; do not run global shortcuts underneath.
            return;
        }

        // Handle Cmd/Ctrl+C for copy
        if key == "c" && is_primary_modifier {
            self.copy_selected_text();
            cx.stop_propagation();
        }
        // Handle Cmd/Ctrl+A for select all on current page
        else if key == "a" && is_primary_modifier {
            self.select_all_text(cx);
            cx.stop_propagation();
        }
        // Handle Escape to clear selection
        else if key == "escape" {
            self.clear_text_selection(cx);
            cx.stop_propagation();
        }
        // Handle Cmd/Ctrl+W to close current tab
        else if key == "w" && is_primary_modifier {
            self.close_current_tab(cx);
            cx.stop_propagation();
        }
        // Handle Cmd/Ctrl+T to toggle command panel
        else if key == "t" && is_primary_modifier && !event.keystroke.modifiers.shift {
            self.toggle_command_panel(window, cx);
            cx.stop_propagation();
        }
        // Handle Cmd/Ctrl+O to open PDF
        else if key == "o" && is_primary_modifier {
            self.open_pdf_dialog(window, cx);
            cx.stop_propagation();
        }
        // Handle Cmd/Ctrl+Shift+[ to switch to previous tab
        else if key == "[" && is_primary_modifier && event.keystroke.modifiers.shift {
            self.switch_visible_tab_by_offset(-1, cx);
            cx.stop_propagation();
        }
        // Handle Cmd/Ctrl+Shift+] to switch to next tab
        else if key == "]" && is_primary_modifier && event.keystroke.modifiers.shift {
            self.switch_visible_tab_by_offset(1, cx);
            cx.stop_propagation();
        }
        // Handle Cmd/Ctrl+1..9 to switch tabs
        else if is_primary_modifier {
            match key {
                "1" => {
                    self.switch_to_visible_tab_by_index(0, cx);
                    cx.stop_propagation();
                }
                "2" => {
                    self.switch_to_visible_tab_by_index(1, cx);
                    cx.stop_propagation();
                }
                "3" => {
                    self.switch_to_visible_tab_by_index(2, cx);
                    cx.stop_propagation();
                }
                "4" => {
                    self.switch_to_visible_tab_by_index(3, cx);
                    cx.stop_propagation();
                }
                "5" => {
                    self.switch_to_visible_tab_by_index(4, cx);
                    cx.stop_propagation();
                }
                "6" => {
                    self.switch_to_visible_tab_by_index(5, cx);
                    cx.stop_propagation();
                }
                "7" => {
                    self.switch_to_visible_tab_by_index(6, cx);
                    cx.stop_propagation();
                }
                "8" => {
                    self.switch_to_visible_tab_by_index(7, cx);
                    cx.stop_propagation();
                }
                "9" => {
                    self.switch_to_visible_tab_by_index(usize::MAX, cx);
                    cx.stop_propagation();
                }
                _ => {}
            }
        }
    }

    fn visible_tab_index_by_id(&self, tab_id: usize) -> Option<usize> {
        let tabs = self.tab_bar.tabs();
        let has_file_open = tabs.iter().any(|tab| tab.path.is_some());
        tabs.iter()
            .filter(|tab| {
                if has_file_open {
                    tab.path.is_some()
                } else {
                    true
                }
            })
            .position(|tab| tab.id == tab_id)
    }

    fn scroll_tab_bar_to_active_tab(&self) {
        let Some(active_tab_id) = self.tab_bar.active_tab_id() else {
            return;
        };

        if let Some(index) = self.visible_tab_index_by_id(active_tab_id) {
            self.tab_bar_scroll.scroll_to_item(index);
        }
    }

    fn remember_recent_file(&mut self, path: &PathBuf) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.clone());
        self.recent_files.truncate(MAX_RECENT_FILES);
        self.persist_recent_files();
    }

    fn persist_recent_files(&self) {
        let Some(store) = self.recent_store.as_ref() else {
            return;
        };

        if store.clear().is_err() {
            return;
        }

        for (ix, path) in self.recent_files.iter().take(MAX_RECENT_FILES).enumerate() {
            let key = (ix as u32).to_be_bytes();
            let value = path.to_string_lossy();
            if store.insert(key, value.as_bytes()).is_err() {
                return;
            }
        }

        let _ = store.flush();
    }

    fn file_position_key(path: &Path) -> Vec<u8> {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .into_owned()
            .into_bytes()
    }

    fn load_saved_file_position(&self, path: &Path) -> Option<usize> {
        let store = self.position_store.as_ref()?;
        let value = store.get(Self::file_position_key(path)).ok().flatten()?;
        if value.len() != 8 {
            return None;
        }

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(value.as_ref());
        usize::try_from(u64::from_be_bytes(bytes)).ok()
    }

    fn save_file_position(&self, path: &Path, page_index: usize) {
        let Some(store) = self.position_store.as_ref() else {
            return;
        };

        let page_bytes = (page_index as u64).to_be_bytes();
        let _ = store.insert(Self::file_position_key(path), page_bytes.as_slice());
        let _ = store.flush();
    }

    fn persist_current_file_position(&mut self) {
        if let Some(tab) = self.active_tab() {
            if tab.pages.is_empty() {
                return;
            }
            let Some(ref path) = tab.path else {
                return;
            };

            let page_index = tab.active_page.min(tab.pages.len().saturating_sub(1));

            if tab
                .last_saved_position
                .as_ref()
                .map(|(saved_path, saved_index)| saved_path == path && *saved_index == page_index)
                .unwrap_or(false)
            {
                return;
            }

            self.save_file_position(path, page_index);
        }
    }

    fn save_window_size(&self, width: f32, height: f32) {
        let Some(store) = self.window_size_store.as_ref() else {
            return;
        };
        let width_bytes = width.to_be_bytes();
        let height_bytes = height.to_be_bytes();
        if store
            .insert(WINDOW_SIZE_KEY_WIDTH, width_bytes.as_slice())
            .is_err()
        {
            crate::debug_log!("[window_size] save width failed");
        }
        if store
            .insert(WINDOW_SIZE_KEY_HEIGHT, height_bytes.as_slice())
            .is_err()
        {
            crate::debug_log!("[window_size] save height failed");
        }
        let _ = store.flush();
    }

    fn recent_popup_open_for(&self, anchor: RecentPopupAnchor) -> bool {
        self.recent_popup_open && self.recent_popup_anchor == Some(anchor)
    }

    fn set_recent_popup_trigger_hovered(
        &mut self,
        anchor: RecentPopupAnchor,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        let mut changed = false;
        match anchor {
            RecentPopupAnchor::OpenButton => {
                if self.recent_popup_trigger_hovered != hovered {
                    self.recent_popup_trigger_hovered = hovered;
                    changed = true;
                }
            }
            RecentPopupAnchor::TabAddButton => {
                if self.recent_popup_tab_trigger_hovered != hovered {
                    self.recent_popup_tab_trigger_hovered = hovered;
                    changed = true;
                }
            }
        }

        if hovered && self.recent_popup_anchor != Some(anchor) {
            self.recent_popup_anchor = Some(anchor);
            changed = true;
        }

        if changed {
            self.update_recent_popup_visibility(cx);
        }
    }

    fn set_recent_popup_panel_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.recent_popup_panel_hovered != hovered {
            self.recent_popup_panel_hovered = hovered;
            self.update_recent_popup_visibility(cx);
        }
    }

    fn update_recent_popup_visibility(&mut self, cx: &mut Context<Self>) {
        if self.recent_popup_trigger_hovered
            || self.recent_popup_tab_trigger_hovered
            || self.recent_popup_panel_hovered
        {
            self.recent_popup_hover_epoch = self.recent_popup_hover_epoch.wrapping_add(1);
            let desired_anchor = if self.recent_popup_tab_trigger_hovered {
                RecentPopupAnchor::TabAddButton
            } else if self.recent_popup_trigger_hovered {
                RecentPopupAnchor::OpenButton
            } else {
                self.recent_popup_anchor
                    .unwrap_or(RecentPopupAnchor::OpenButton)
            };

            let mut changed = false;
            if self.recent_popup_anchor != Some(desired_anchor) {
                self.recent_popup_anchor = Some(desired_anchor);
                changed = true;
            }

            if !self.recent_popup_open {
                self.recent_popup_open = true;
                changed = true;
            }

            if changed {
                cx.notify();
            }
            return;
        }

        self.recent_popup_hover_epoch = self.recent_popup_hover_epoch.wrapping_add(1);
        let close_epoch = self.recent_popup_hover_epoch;

        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(RECENT_POPUP_CLOSE_DELAY_MS))
                .await;

            let _ = view.update(cx, |this, cx| {
                if this.recent_popup_hover_epoch != close_epoch {
                    return;
                }
                if this.recent_popup_trigger_hovered
                    || this.recent_popup_tab_trigger_hovered
                    || this.recent_popup_panel_hovered
                {
                    return;
                }
                if this.recent_popup_open {
                    this.recent_popup_open = false;
                    this.recent_popup_anchor = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn close_recent_popup(&mut self, cx: &mut Context<Self>) {
        self.recent_popup_hover_epoch = self.recent_popup_hover_epoch.wrapping_add(1);

        let mut has_changed = false;
        if self.recent_popup_trigger_hovered {
            self.recent_popup_trigger_hovered = false;
            has_changed = true;
        }
        if self.recent_popup_tab_trigger_hovered {
            self.recent_popup_tab_trigger_hovered = false;
            has_changed = true;
        }
        if self.recent_popup_panel_hovered {
            self.recent_popup_panel_hovered = false;
            has_changed = true;
        }
        if self.recent_popup_open {
            self.recent_popup_open = false;
            has_changed = true;
        }
        if self.recent_popup_anchor.is_some() {
            self.recent_popup_anchor = None;
            has_changed = true;
        }
        if has_changed {
            cx.notify();
        }
    }

    fn recent_files_with_positions(
        &self,
        recent_files: &[PathBuf],
    ) -> Vec<(PathBuf, Option<usize>)> {
        recent_files
            .iter()
            .cloned()
            .map(|path| {
                let last_seen = self.load_saved_file_position(&path);
                (path, last_seen)
            })
            .collect()
    }

    fn current_bookmark_entry(&self) -> Option<BookmarkEntry> {
        let tab = self.active_tab()?;
        let path = tab.path.clone()?;
        if tab.pages.is_empty() {
            return None;
        }
        Some(BookmarkEntry {
            path,
            page_index: tab.active_page.min(tab.pages.len().saturating_sub(1)),
            created_at_unix_secs: Self::now_unix_secs(),
        })
    }

    fn insert_bookmark(&mut self, entry: BookmarkEntry) {
        self.bookmarks
            .retain(|item| !(item.path == entry.path && item.page_index == entry.page_index));
        self.bookmarks.insert(0, entry);
        self.persist_bookmarks();
    }

    pub(super) fn add_current_page_bookmark_and_open(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.current_bookmark_entry() {
            self.insert_bookmark(entry);
        }

        if self.recent_popup_open {
            self.close_recent_popup(cx);
        }

        self.bookmark_scope = if self.active_tab_path().is_some() {
            BookmarkScope::CurrentPdf
        } else {
            BookmarkScope::All
        };
        self.bookmark_popup_list_scroll.scroll_to_item(0);
        self.bookmark_popup_hover_epoch = self.bookmark_popup_hover_epoch.wrapping_add(1);
        self.bookmark_popup_open = true;
        cx.notify();
    }

    pub(super) fn close_bookmark_popup(&mut self, cx: &mut Context<Self>) {
        self.bookmark_popup_hover_epoch = self.bookmark_popup_hover_epoch.wrapping_add(1);

        let mut has_changed = false;
        if self.bookmark_popup_trigger_hovered {
            self.bookmark_popup_trigger_hovered = false;
            has_changed = true;
        }
        if self.bookmark_popup_panel_hovered {
            self.bookmark_popup_panel_hovered = false;
            has_changed = true;
        }
        if self.bookmark_popup_open {
            self.bookmark_popup_open = false;
            has_changed = true;
        }
        if has_changed {
            cx.notify();
        }
    }

    pub(super) fn set_bookmark_popup_trigger_hovered(
        &mut self,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if self.bookmark_popup_trigger_hovered == hovered {
            return;
        }
        self.bookmark_popup_trigger_hovered = hovered;
        self.update_bookmark_popup_visibility(cx);
    }

    pub(super) fn set_bookmark_popup_panel_hovered(
        &mut self,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if self.bookmark_popup_panel_hovered == hovered {
            return;
        }
        self.bookmark_popup_panel_hovered = hovered;
        self.update_bookmark_popup_visibility(cx);
    }

    fn update_bookmark_popup_visibility(&mut self, cx: &mut Context<Self>) {
        if self.bookmark_popup_trigger_hovered || self.bookmark_popup_panel_hovered {
            self.bookmark_popup_hover_epoch = self.bookmark_popup_hover_epoch.wrapping_add(1);
            if !self.bookmark_popup_open {
                self.bookmark_scope = if self.active_tab_path().is_some() {
                    BookmarkScope::CurrentPdf
                } else {
                    BookmarkScope::All
                };
                self.bookmark_popup_list_scroll.scroll_to_item(0);
                self.bookmark_popup_open = true;
                cx.notify();
            }
            return;
        }

        self.bookmark_popup_hover_epoch = self.bookmark_popup_hover_epoch.wrapping_add(1);
        let close_epoch = self.bookmark_popup_hover_epoch;

        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(BOOKMARK_POPUP_CLOSE_DELAY_MS))
                .await;

            let _ = view.update(cx, |this, cx| {
                if this.bookmark_popup_hover_epoch != close_epoch {
                    return;
                }
                if this.bookmark_popup_trigger_hovered || this.bookmark_popup_panel_hovered {
                    return;
                }
                if this.bookmark_popup_open {
                    this.bookmark_popup_open = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn set_bookmark_scope(&mut self, scope: BookmarkScope, cx: &mut Context<Self>) {
        if self.bookmark_scope == scope {
            return;
        }
        self.bookmark_scope = scope;
        self.bookmark_popup_list_scroll.scroll_to_item(0);
        cx.notify();
    }

    pub(super) fn bookmarks_for_scope(&self, scope: BookmarkScope) -> Vec<BookmarkEntry> {
        let current_path = self.active_tab_path();
        self.bookmarks
            .iter()
            .filter(|item| match scope {
                BookmarkScope::All => true,
                BookmarkScope::CurrentPdf => current_path == Some(&item.path),
            })
            .cloned()
            .collect()
    }

    pub(super) fn active_tab_page_bookmarked(&self, page_index: usize) -> bool {
        let Some(path) = self.active_tab_path() else {
            return false;
        };
        self.bookmarks
            .iter()
            .any(|item| item.path == *path && item.page_index == page_index)
    }

    fn open_bookmark(&mut self, bookmark: BookmarkEntry, cx: &mut Context<Self>) {
        if !bookmark.path.exists() {
            let original_len = self.bookmarks.len();
            self.bookmarks.retain(|item| item.path != bookmark.path);
            if self.bookmarks.len() != original_len {
                self.persist_bookmarks();
                cx.notify();
            }
            return;
        }

        self.save_file_position(&bookmark.path, bookmark.page_index);

        let existing_tab_id = self
            .tab_bar
            .tabs()
            .iter()
            .find(|tab| tab.path.as_ref() == Some(&bookmark.path))
            .map(|tab| tab.id);

        if let Some(tab_id) = existing_tab_id {
            self.switch_to_tab(tab_id, cx);
            if let Some(tab) = self.active_tab()
                && tab.path.as_ref() == Some(&bookmark.path)
                && !tab.pages.is_empty()
            {
                let target = bookmark.page_index.min(tab.pages.len().saturating_sub(1));
                self.select_page(target, cx);
            }
            self.close_bookmark_popup(cx);
            return;
        }

        self.open_recent_pdf(bookmark.path, cx);
        self.close_bookmark_popup(cx);
    }

    fn delete_bookmark(&mut self, bookmark: &BookmarkEntry, cx: &mut Context<Self>) {
        let original_len = self.bookmarks.len();
        self.bookmarks
            .retain(|item| !(item.path == bookmark.path && item.page_index == bookmark.page_index));
        if self.bookmarks.len() != original_len {
            self.persist_bookmarks();
            cx.notify();
        }
    }

    pub(super) fn render_bookmark_popup_panel(
        popup_id: &'static str,
        i18n: I18n,
        viewer: Entity<Self>,
        scope: BookmarkScope,
        bookmarks: Vec<BookmarkEntry>,
        scroll_handle: &ScrollHandle,
        cx: &mut Context<PopoverState>,
    ) -> AnyElement {
        let now_unix_secs = Self::now_unix_secs();
        div()
            .id(popup_id)
            .relative()
            .top(px(-1.))
            .w(px(340.))
            .v_flex()
            .gap_2()
            .popover_style(cx)
            .p_2()
            .on_hover({
                let viewer = viewer.clone();
                move |hovered, _, cx| {
                    let _ = viewer.update(cx, |this, cx| {
                        this.set_bookmark_popup_panel_hovered(*hovered, cx);
                    });
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div().flex_1().child(
                            Button::new("bookmark-scope-current")
                                .small()
                                .w_full()
                                .label(i18n.bookmark_scope_current_pdf())
                                .when(scope != BookmarkScope::CurrentPdf, |this| this.ghost())
                                .on_click({
                                    let viewer = viewer.clone();
                                    move |_, _, cx| {
                                        let _ = viewer.update(cx, |this, cx| {
                                            this.set_bookmark_scope(BookmarkScope::CurrentPdf, cx);
                                        });
                                    }
                                }),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            Button::new("bookmark-scope-all")
                                .small()
                                .w_full()
                                .label(i18n.bookmark_scope_all())
                                .when(scope != BookmarkScope::All, |this| this.ghost())
                                .on_click({
                                    let viewer = viewer.clone();
                                    move |_, _, cx| {
                                        let _ = viewer.update(cx, |this, cx| {
                                            this.set_bookmark_scope(BookmarkScope::All, cx);
                                        });
                                    }
                                }),
                        ),
                    ),
            )
            .child(div().h(px(1.)).bg(cx.theme().border))
            .when(bookmarks.is_empty(), |this| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(i18n.no_bookmarks()),
                )
            })
            .when(!bookmarks.is_empty(), |this| {
                this.child(
                    div()
                        .id("bookmark-list-scroll-wrap")
                        .w_full()
                        .max_h(px(RECENT_FILES_LIST_MAX_HEIGHT))
                        .relative()
                        .child(
                            div()
                                .id("bookmark-list-scroll")
                                .w_full()
                                .max_h(px(RECENT_FILES_LIST_MAX_HEIGHT))
                                .overflow_y_scroll()
                                .track_scroll(scroll_handle)
                                .pr(px(10.))
                                .v_flex()
                                .gap_1()
                                .children(
                                    bookmarks
                                        .iter()
                                        .enumerate()
                                        .map(|(ix, bookmark)| {
                                            let bookmark = bookmark.clone();
                                            let bookmark_for_delete = bookmark.clone();
                                            let bookmark_for_open = bookmark.clone();
                                            let file_name = display_file_name(&bookmark.path);
                                            let page_label =
                                                i18n.bookmark_page_label(bookmark.page_index + 1);
                                            let path_text = bookmark.path.display().to_string();
                                            let added_time_label =
                                                if bookmark.created_at_unix_secs == 0 {
                                                    i18n.bookmark_added_unknown().to_string()
                                                } else {
                                                    i18n.bookmark_added_relative(
                                                        now_unix_secs.saturating_sub(
                                                            bookmark.created_at_unix_secs,
                                                        ),
                                                    )
                                                };
                                            div()
                                                .id(("bookmark-item", ix))
                                                .w_full()
                                                .rounded_md()
                                                .px_2()
                                                .py_1()
                                                .cursor_pointer()
                                                .hover(|this| {
                                                    this.bg(cx.theme().secondary.opacity(0.6))
                                                })
                                                .active(|this| {
                                                    this.bg(cx.theme().secondary.opacity(0.9))
                                                })
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .flex()
                                                        .items_start()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .v_flex()
                                                                .items_start()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .text_sm()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .popover_foreground,
                                                                        )
                                                                        .child(page_label),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .whitespace_normal()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .child(file_name),
                                                                )
                                                                .when(
                                                                    scope == BookmarkScope::All,
                                                                    |this| {
                                                                        this.child(
                                                                            div()
                                                                                .w_full()
                                                                                .whitespace_normal()
                                                                                .text_xs()
                                                                                .text_color(
                                                                                    cx.theme()
                                                                                        .muted_foreground,
                                                                                )
                                                                                .child(path_text),
                                                                        )
                                                                    },
                                                                )
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .whitespace_normal()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .child(added_time_label),
                                                                ),
                                                        )
                                                        .child(
                                                            Button::new(("bookmark-delete", ix))
                                                                .xsmall()
                                                                .ghost()
                                                                .icon(
                                                                    Icon::new(
                                                                        crate::icons::IconName::BookmarkMinus,
                                                                    )
                                                                    .size_4()
                                                                    .text_color(
                                                                        cx.theme().muted_foreground,
                                                                    ),
                                                                )
                                                                .on_click({
                                                                    let viewer = viewer.clone();
                                                                    move |_, _, cx| {
                                                                        let bookmark =
                                                                            bookmark_for_delete
                                                                                .clone();
                                                                        let _ = viewer.update(
                                                                            cx,
                                                                            |this, cx| {
                                                                                this.delete_bookmark(
                                                                                    &bookmark, cx,
                                                                                );
                                                                            },
                                                                        );
                                                                    }
                                                                }),
                                                        ),
                                                )
                                                .on_click({
                                                    let viewer = viewer.clone();
                                                    move |_, _, cx| {
                                                        let bookmark = bookmark_for_open.clone();
                                                        let _ = viewer.update(cx, |this, cx| {
                                                            this.open_bookmark(bookmark, cx);
                                                        });
                                                    }
                                                })
                                                .into_any_element()
                                        })
                                        .collect::<Vec<_>>(),
                                ),
                        )
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .child(
                                    Scrollbar::vertical(scroll_handle)
                                        .scrollbar_show(ScrollbarShow::Always),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_recent_files_list_content(
        list_key: usize,
        i18n: I18n,
        viewer: Entity<Self>,
        recent_files_with_positions: Vec<(PathBuf, Option<usize>)>,
        scroll_handle: &ScrollHandle,
        show_choose_file_button: bool,
        cx: &App,
    ) -> AnyElement {
        div()
            .w_full()
            .v_flex()
            .gap_1()
            .when(show_choose_file_button, |this| {
                this.child(
                    Button::new(("open-pdf-dialog", list_key))
                        .small()
                        .w_full()
                        .icon(
                            Icon::new(crate::icons::IconName::FolderOpen)
                                .text_color(cx.theme().foreground),
                        )
                        .label(i18n.choose_file_button())
                        .on_click({
                            let viewer = viewer.clone();
                            move |_, window, cx| {
                                let _ = viewer.update(cx, |this, cx| {
                                    this.close_recent_popup(cx);
                                    this.open_pdf_dialog(window, cx);
                                });
                            }
                        }),
                )
                .child(div().h(px(1.)).my_1().bg(cx.theme().border))
            })
            .when(recent_files_with_positions.is_empty(), |this| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(i18n.no_recent_files()),
                )
            })
            .when(!recent_files_with_positions.is_empty(), |this| {
                this.child(
                    div()
                        .id(("recent-files-scroll-wrap", list_key))
                        .w_full()
                        .max_h(px(RECENT_FILES_LIST_MAX_HEIGHT))
                        .relative()
                        .child(
                            div()
                                .id(("recent-files-scroll", list_key))
                                .w_full()
                                .max_h(px(RECENT_FILES_LIST_MAX_HEIGHT))
                                .overflow_y_scroll()
                                .track_scroll(scroll_handle)
                                .pr(px(10.))
                                .v_flex()
                                .gap_1()
                                .children(
                                    recent_files_with_positions
                                        .iter()
                                        .enumerate()
                                        .map(|(ix, (path, last_seen_page))| {
                                            let path = path.clone();
                                            let file_name = display_file_name(&path);
                                            let path_text = path.display().to_string();
                                            let last_seen_text = last_seen_page.map(|page_index| {
                                                i18n.last_seen_page(page_index + 1)
                                            });
                                            div()
                                                .id((
                                                    "recent-pdf",
                                                    list_key * MAX_RECENT_FILES + ix,
                                                ))
                                                .w_full()
                                                .rounded_md()
                                                .px_2()
                                                .py_1()
                                                .cursor_pointer()
                                                .hover(|this| {
                                                    this.bg(cx.theme().secondary.opacity(0.6))
                                                })
                                                .active(|this| {
                                                    this.bg(cx.theme().secondary.opacity(0.9))
                                                })
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .v_flex()
                                                        .items_start()
                                                        .gap_1()
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .whitespace_normal()
                                                                .text_sm()
                                                                .text_color(
                                                                    cx.theme().popover_foreground,
                                                                )
                                                                .child(file_name),
                                                        )
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .whitespace_normal()
                                                                .text_xs()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                )
                                                                .child(path_text),
                                                        )
                                                        .when_some(
                                                            last_seen_text,
                                                            |this, label| {
                                                                this.child(
                                                                    div()
                                                                        .w_full()
                                                                        .whitespace_normal()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .child(label),
                                                                )
                                                            },
                                                        ),
                                                )
                                                .on_click({
                                                    let viewer = viewer.clone();
                                                    move |_, _, cx| {
                                                        let _ = viewer.update(cx, |this, cx| {
                                                            this.close_recent_popup(cx);
                                                            this.open_recent_pdf(path.clone(), cx);
                                                        });
                                                    }
                                                })
                                                .into_any_element()
                                        })
                                        .collect::<Vec<_>>(),
                                ),
                        )
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .child(
                                    Scrollbar::vertical(scroll_handle)
                                        .scrollbar_show(ScrollbarShow::Always),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_recent_files_popup_panel(
        popup_id: &'static str,
        popup_key: usize,
        i18n: I18n,
        viewer: Entity<Self>,
        recent_files_with_positions: Vec<(PathBuf, Option<usize>)>,
        scroll_handle: &ScrollHandle,
        cx: &mut Context<PopoverState>,
    ) -> AnyElement {
        div()
            .id(popup_id)
            .relative()
            .top(px(-1.))
            .w(px(340.))
            .v_flex()
            .gap_1()
            .popover_style(cx)
            .p_2()
            .on_hover({
                let viewer = viewer.clone();
                move |hovered, _, cx| {
                    let _ = viewer.update(cx, |this, cx| {
                        this.set_recent_popup_panel_hovered(*hovered, cx);
                    });
                }
            })
            .child(Self::render_recent_files_list_content(
                popup_key,
                i18n,
                viewer,
                recent_files_with_positions,
                scroll_handle,
                true,
                cx,
            ))
            .into_any_element()
    }

    fn select_page(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            if index < tab.pages.len() {
                tab.selected_page = index;
                tab.active_page = index;
                self.sync_scroll_to_selected();
                self.persist_current_file_position();
                cx.notify();
            }
        }
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            if tab.active_page > 0 {
                let new_page = tab.active_page - 1;
                self.select_page(new_page, cx);
            }
        }
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            if tab.active_page + 1 < tab.pages.len() {
                let new_page = tab.active_page + 1;
                self.select_page(new_page, cx);
            }
        }
    }

    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.zoom = (tab.zoom + ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
            cx.notify();
        }
    }

    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.zoom = (tab.zoom - ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
            cx.notify();
        }
    }

    fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.zoom = 1.0;
            cx.notify();
        }
    }

    fn sync_scroll_to_selected(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.suppress_display_scroll_sync_once = true;
            tab.thumbnail_scroll
                .scroll_to_item(tab.selected_page, ScrollStrategy::Center);
            tab.display_scroll
                .scroll_to_item(tab.selected_page, ScrollStrategy::Top);
        }
    }

    fn schedule_thumbnail_sync_after_display_scroll(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };

        tab.display_scroll_sync_epoch = tab.display_scroll_sync_epoch.wrapping_add(1);
        let sync_epoch = tab.display_scroll_sync_epoch;
        let tab_id = tab.id;

        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(DISPLAY_SCROLL_SYNC_DELAY_MS))
                .await;

            let _ = view.update(cx, |this, cx| {
                let path_to_save = {
                    let Some(tab) = this.tab_bar.get_active_tab_mut() else {
                        return;
                    };
                    if tab.id != tab_id
                        || tab.display_scroll_sync_epoch != sync_epoch
                        || tab.pages.is_empty()
                    {
                        return;
                    }

                    let next_active = tab
                        .last_display_visible_range
                        .as_ref()
                        .map(|range| range.start.min(tab.pages.len().saturating_sub(1)))
                        .unwrap_or_else(|| tab.active_page.min(tab.pages.len().saturating_sub(1)));

                    let page_index_changed = tab.active_page != next_active;

                    if page_index_changed {
                        tab.active_page = next_active;
                        // Save position directly
                        if let Some(ref path) = tab.path {
                            if !tab.pages.is_empty() {
                                let page_index =
                                    tab.active_page.min(tab.pages.len().saturating_sub(1));
                                tab.last_saved_position = Some((path.clone(), page_index));
                                Some((path.clone(), page_index))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                // Save file position outside the mutable borrow
                if let Some((path, page_index)) = path_to_save {
                    this.save_file_position(&path, page_index);
                }

                // Get tab again for scroll operation
                if let Some(tab) = this.tab_bar.get_active_tab_mut() {
                    let next_active = tab.active_page;
                    tab.thumbnail_scroll
                        .scroll_to_item(next_active, ScrollStrategy::Center);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn on_display_scroll_offset_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            let offset = tab.display_scroll.offset();
            let has_changed = tab
                .last_display_scroll_offset
                .map(|last| last != offset)
                .unwrap_or(false);
            tab.last_display_scroll_offset = Some(offset);

            if has_changed && !tab.pages.is_empty() {
                if tab.suppress_display_scroll_sync_once {
                    tab.suppress_display_scroll_sync_once = false;
                    return;
                }
                self.schedule_thumbnail_sync_after_display_scroll(cx);
            }
        }
    }

    fn schedule_restore_current_page_after_layout_change(
        &mut self,
        keep_page: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self.tab_bar.active_tab_id() else {
            return;
        };

        self.resize_restore_epoch = self.resize_restore_epoch.wrapping_add(1);
        let restore_epoch = self.resize_restore_epoch;

        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;

            let _ = view.update(cx, |this, cx| {
                if this.resize_restore_epoch != restore_epoch {
                    return;
                }

                let Some(tab) = this.tab_bar.get_active_tab_mut() else {
                    return;
                };
                if tab.id != tab_id || tab.pages.is_empty() {
                    return;
                }

                let page_index = keep_page.min(tab.pages.len().saturating_sub(1));
                tab.active_page = page_index;
                tab.selected_page = page_index;
                tab.last_display_visible_range =
                    Some(page_index..page_index.saturating_add(1).min(tab.pages.len()));
                tab.suppress_display_scroll_sync_once = true;
                tab.display_scroll
                    .scroll_to_item(page_index, ScrollStrategy::Top);
                tab.thumbnail_scroll
                    .scroll_to_item(page_index, ScrollStrategy::Center);
                tab.last_display_scroll_offset = Some(tab.display_scroll.offset());
                cx.notify();
            });
        })
        .detach();
    }

    fn thumbnail_base_width(&self) -> f32 {
        (SIDEBAR_WIDTH - THUMB_HORIZONTAL_PADDING).max(THUMB_MIN_WIDTH)
    }

    fn thumbnail_card_size(&self, page: &PageSummary) -> (f32, f32) {
        let width = self.thumbnail_base_width();
        let aspect_ratio = if page.width_pt > 1.0 {
            page.height_pt / page.width_pt
        } else {
            1.4
        };
        let height = width * aspect_ratio;
        (width, height)
    }

    fn thumbnail_row_height(&self, page: &PageSummary) -> f32 {
        let (_, height) = self.thumbnail_card_size(page);
        height + THUMB_VERTICAL_PADDING
    }

    fn thumbnail_item_sizes(&self, pages: &[PageSummary]) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            pages
                .iter()
                .map(|page| size(px(0.), px(self.thumbnail_row_height(page))))
                .collect(),
        )
    }

    fn thumbnail_target_width(&self, window: &Window) -> u32 {
        let width = self.thumbnail_base_width() * window.scale_factor();
        width.clamp(1.0, i32::MAX as f32).round() as u32
    }

    fn request_thumbnail_load_from_candidates(
        &mut self,
        candidate_order: Vec<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        let language = self.language;
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if candidate_order.is_empty() || tab.pages.is_empty() {
            return;
        }

        let Some(path) = tab.path.clone() else {
            return;
        };

        if tab.thumbnail_inflight_tasks >= THUMB_MAX_PARALLEL_TASKS {
            return;
        }

        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for ix in candidate_order {
            if !seen.insert(ix) {
                continue;
            }

            let Some(page) = tab.pages.get(ix) else {
                continue;
            };

            let needs_render =
                page.thumbnail_image.is_none() || page.thumbnail_render_width < target_width;
            if needs_render && !page.thumbnail_failed {
                pending.push(ix);
                if pending.len() >= THUMB_BATCH_SIZE {
                    break;
                }
            }
        }

        if pending.is_empty() {
            return;
        }

        for ix in &pending {
            tab.thumbnail_loading.insert(*ix);
        }
        tab.thumbnail_inflight_tasks = tab.thumbnail_inflight_tasks.saturating_add(1);
        let epoch = tab.thumbnail_epoch;
        let tab_id = tab.id;

        cx.spawn(async move |view, cx| {
            let load_result = cx
                .background_executor()
                .spawn(async move {
                    let loaded = load_display_images(&path, &pending, target_width, language);
                    (pending, target_width, loaded)
                })
                .await;

            let _ = view.update(cx, |this, cx| {
                let Some(tab) = this.tab_bar.get_active_tab_mut() else {
                    return;
                };
                if tab.id != tab_id || tab.thumbnail_epoch != epoch {
                    return;
                }

                tab.thumbnail_inflight_tasks = tab.thumbnail_inflight_tasks.saturating_sub(1);

                let (requested_indices, loaded_target_width, loaded_result) = load_result;
                let mut loaded_indices = HashSet::new();

                match loaded_result {
                    Ok(images) => {
                        for (ix, image) in images {
                            if let Some(page) = tab.pages.get_mut(ix) {
                                page.thumbnail_image = Some(image);
                                page.thumbnail_render_width = loaded_target_width;
                                page.thumbnail_failed = false;
                                loaded_indices.insert(ix);
                            }
                        }
                    }
                    Err(_) => {}
                }

                for ix in requested_indices {
                    tab.thumbnail_loading.remove(&ix);
                    if !loaded_indices.contains(&ix)
                        && let Some(page) = tab.pages.get_mut(ix)
                    {
                        page.thumbnail_failed = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_thumbnail_load_for_visible_range(
        &mut self,
        visible_range: std::ops::Range<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if visible_range.is_empty() || tab.pages.is_empty() {
            return;
        }

        if tab.thumbnail_inflight_tasks == 0 && !tab.thumbnail_loading.is_empty() {
            tab.thumbnail_loading.clear();
        }

        tab.last_thumbnail_visible_range = Some(visible_range.clone());

        let mut candidate_order = Vec::with_capacity(visible_range.len());
        candidate_order.extend(visible_range.clone());
        self.request_thumbnail_load_from_candidates(candidate_order, target_width, cx);
    }

    fn display_available_width(&self, window: &Window) -> f32 {
        let viewport_width: f32 = window.viewport_size().width.into();
        let sidebar_width = if self.show_thumbnail_panel() {
            SIDEBAR_WIDTH
        } else {
            0.0
        };
        (viewport_width - sidebar_width).max(DISPLAY_MIN_WIDTH)
    }

    fn display_panel_width(&self, window: &Window, zoom: f32) -> f32 {
        let available_width = self.display_available_width(window);
        (available_width * zoom).clamp(DISPLAY_MIN_WIDTH, available_width)
    }

    fn display_base_width(&self, window: &Window, zoom: f32) -> f32 {
        self.display_panel_width(window, zoom)
    }

    fn display_card_size(&self, page: &PageSummary, base_width: f32) -> (f32, f32) {
        let width = base_width.max(DISPLAY_MIN_WIDTH);
        let aspect_ratio = if page.width_pt > 1.0 {
            page.height_pt / page.width_pt
        } else {
            1.4
        };
        let height = width * aspect_ratio;
        (width, height)
    }

    fn display_row_height(&self, page: &PageSummary, base_width: f32) -> f32 {
        let (_, height) = self.display_card_size(page, base_width);
        height
    }

    fn display_item_sizes(
        &self,
        pages: &[PageSummary],
        base_width: f32,
    ) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            pages
                .iter()
                .map(|page| size(px(0.), px(self.display_row_height(page, base_width))))
                .collect(),
        )
    }

    fn display_target_width(&self, window: &Window, zoom: f32) -> u32 {
        let width = self.display_panel_width(window, zoom) * window.scale_factor();
        width.clamp(1.0, i32::MAX as f32).round() as u32
    }

    fn request_display_load_from_candidates(
        &mut self,
        candidate_order: Vec<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        let language = self.language;
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if candidate_order.is_empty() || tab.pages.is_empty() {
            return;
        }

        let Some(path) = tab.path.clone() else {
            return;
        };

        if tab.display_inflight_tasks >= DISPLAY_MAX_PARALLEL_TASKS {
            return;
        }

        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for ix in candidate_order {
            if !seen.insert(ix) {
                continue;
            }

            let Some(page) = tab.pages.get(ix) else {
                continue;
            };

            let needs_render =
                page.display_image.is_none() || page.display_render_width < target_width;
            if needs_render && !page.display_failed {
                pending.push(ix);
                if pending.len() >= DISPLAY_BATCH_SIZE {
                    break;
                }
            }
        }

        if pending.is_empty() {
            return;
        }

        for ix in &pending {
            tab.display_loading.insert(*ix);
        }
        tab.display_inflight_tasks = tab.display_inflight_tasks.saturating_add(1);
        let epoch = tab.display_epoch;
        let tab_id = tab.id;

        cx.spawn(async move |view, cx| {
            let load_result = cx
                .background_executor()
                .spawn(async move {
                    let loaded = load_display_images(&path, &pending, target_width, language);
                    (pending, target_width, loaded)
                })
                .await;

            let _ = view.update(cx, |this, cx| {
                let Some(tab) = this.tab_bar.get_active_tab_mut() else {
                    return;
                };
                if tab.id != tab_id || tab.display_epoch != epoch {
                    return;
                }

                tab.display_inflight_tasks = tab.display_inflight_tasks.saturating_sub(1);

                let (requested_indices, loaded_target_width, loaded_result) = load_result;
                let mut loaded_indices = HashSet::new();

                match loaded_result {
                    Ok(images) => {
                        for (ix, image) in images {
                            if let Some(page) = tab.pages.get_mut(ix) {
                                page.display_image = Some(image);
                                page.display_render_width = loaded_target_width;
                                page.display_failed = false;
                                loaded_indices.insert(ix);
                            }
                        }
                    }
                    Err(_) => {}
                }

                for ix in requested_indices {
                    tab.display_loading.remove(&ix);
                    if !loaded_indices.contains(&ix)
                        && let Some(page) = tab.pages.get_mut(ix)
                    {
                        page.display_failed = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_display_load_for_visible_range(
        &mut self,
        visible_range: std::ops::Range<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if visible_range.is_empty() || tab.pages.is_empty() {
            return;
        }

        if tab.display_inflight_tasks == 0 && !tab.display_loading.is_empty() {
            tab.display_loading.clear();
        }

        tab.last_display_visible_range = Some(visible_range.clone());

        let mut candidate_order = Vec::with_capacity(visible_range.len());
        candidate_order.extend(visible_range.clone());

        self.request_display_load_from_candidates(candidate_order, target_width, cx);
    }

    pub fn copy_selected_text(&self) {
        if let Some(tab) = self.active_tab() {
            let manager = tab.text_selection_manager.borrow();
            if let Some(text) = manager.get_selected_text() {
                if !text.is_empty() {
                    if let Err(err) = copy_to_clipboard(&text) {
                        crate::debug_log!("[copy] failed to copy to clipboard: {}", err);
                    }
                }
            }
        }
    }

    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            if tab.pages.get(tab.active_page).is_some() {
                let mut manager = tab.text_selection_manager.borrow_mut();
                if let Some(cache) = manager.get_page_cache(tab.active_page) {
                    let char_count = cache.chars.len();
                    if char_count > 0 {
                        manager.start_selection(tab.active_page, 0);
                        manager.update_selection(tab.active_page, char_count);
                        manager.end_selection();
                        cx.notify();
                    }
                }
            }
        }
    }

    pub fn clear_text_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.text_selection_manager.borrow_mut().clear_selection();
        }
        cx.notify();
    }

    pub fn open_context_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.context_menu_open = true;
        self.context_menu_position = Some(position);
        self.context_menu_tab_id = None;
        cx.notify();
    }

    pub fn open_tab_context_menu(
        &mut self,
        tab_id: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.tab_bar.get_tab_index_by_id(tab_id).is_none() {
            return;
        }

        #[cfg(target_os = "macos")]
        {
            let _ = position;
            let i18n = self.i18n();
            let can_close_others = self.tab_bar.tabs().len() > 1;
            let can_reveal = self
                .tab_bar
                .tabs()
                .iter()
                .any(|tab| tab.id == tab_id && tab.path.is_some());
            self.close_context_menu(cx);
            if let Some(action) = self::macos_context_menu::show_tab_context_menu(
                i18n.close_all_tabs_button(),
                i18n.close_other_tabs_button(),
                i18n.reveal_in_file_manager_button(),
                can_close_others,
                can_reveal,
            ) {
                match action {
                    self::macos_context_menu::MacTabContextMenuAction::CloseAllTabs => {
                        self.close_all_tabs(cx);
                    }
                    self::macos_context_menu::MacTabContextMenuAction::CloseOtherTabs => {
                        self.close_other_tabs(tab_id, cx);
                    }
                    self::macos_context_menu::MacTabContextMenuAction::RevealInFinder => {
                        self.reveal_tab_in_file_manager(tab_id);
                    }
                }
            }
            return;
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.context_menu_open = true;
            self.context_menu_position = Some(position);
            self.context_menu_tab_id = Some(tab_id);
            cx.notify();
        }
    }

    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if !self.context_menu_open
            && self.context_menu_position.is_none()
            && self.context_menu_tab_id.is_none()
        {
            return;
        }

        self.context_menu_open = false;
        self.context_menu_position = None;
        self.context_menu_tab_id = None;
        cx.notify();
    }

    pub fn has_text_selection(&self) -> bool {
        self.active_tab()
            .and_then(|tab| tab.text_selection_manager.borrow().get_selected_text())
            .is_some()
    }

    pub(super) fn set_text_hover_hit(&mut self, page_index: usize, is_over_text: bool) -> bool {
        let next = if is_over_text {
            self.tab_bar
                .active_tab_id()
                .map(|tab_id| (tab_id, page_index))
        } else {
            None
        };

        if self.text_hover_target != next {
            self.text_hover_target = next;
            true
        } else {
            false
        }
    }

    pub(super) fn text_cursor_style_for_page(&self, page_index: usize) -> gpui::CursorStyle {
        let target = self
            .tab_bar
            .active_tab_id()
            .map(|tab_id| (tab_id, page_index));
        if self.text_hover_target == target {
            gpui::CursorStyle::IBeam
        } else {
            gpui::CursorStyle::Arrow
        }
    }

    pub fn close_current_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(active_id) = self.tab_bar.active_tab_id() {
            self.close_tab(active_id, cx);
        }
    }

    // Convenience methods for accessing active tab data in render functions
    pub(super) fn active_tab_display_scroll(
        &self,
    ) -> Option<&gpui_component::VirtualListScrollHandle> {
        self.active_tab().map(|t| &t.display_scroll)
    }

    pub(super) fn active_tab_thumbnail_scroll(
        &self,
    ) -> Option<&gpui_component::VirtualListScrollHandle> {
        self.active_tab().map(|t| &t.thumbnail_scroll)
    }

    pub(super) fn active_tab_pages(&self) -> Option<&Vec<PageSummary>> {
        self.active_tab().map(|t| &t.pages)
    }

    pub(super) fn active_tab_zoom(&self) -> f32 {
        self.active_tab().map(|t| t.zoom).unwrap_or(1.0)
    }

    pub(super) fn active_tab_active_page(&self) -> usize {
        self.active_tab().map(|t| t.active_page).unwrap_or(0)
    }

    #[allow(dead_code)]
    pub(super) fn active_tab_selected_page(&self) -> usize {
        self.active_tab().map(|t| t.selected_page).unwrap_or(0)
    }

    pub(super) fn active_tab_text_selection_manager(
        &self,
    ) -> Option<&std::cell::RefCell<crate::pdf_viewer::text_selection::TextSelectionManager>> {
        self.active_tab().map(|t| &t.text_selection_manager)
    }

    pub(super) fn active_tab_path(&self) -> Option<&PathBuf> {
        self.active_tab().and_then(|t| t.path.as_ref())
    }

    fn show_thumbnail_panel(&self) -> bool {
        self.active_tab_path().is_some()
    }

    fn current_drag_source_tab_id(&self) -> Option<usize> {
        match self.drag_state {
            DragState::Started { source_tab_id } => Some(source_tab_id),
            DragState::Over { source_tab_id, .. } => Some(source_tab_id),
            _ => None,
        }
    }

    fn update_drag_mouse_position(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(source_tab_id) = self.current_drag_source_tab_id() else {
            return;
        };

        let mut should_notify = false;
        if self.drag_mouse_position != Some(position) {
            self.drag_mouse_position = Some(position);
            should_notify = true;
        }

        // When cursor leaves tab bar band, clear stale target to avoid "stuck" drag feedback.
        let y: f32 = position.y.into();
        let tab_bar_bottom = TITLE_BAR_HEIGHT + TAB_BAR_HEIGHT;
        if !(TITLE_BAR_HEIGHT..=tab_bar_bottom).contains(&y)
            && !matches!(self.drag_state, DragState::Started { source_tab_id: id } if id == source_tab_id)
        {
            self.drag_state = DragState::Started { source_tab_id };
            should_notify = true;
        }

        if should_notify {
            cx.notify();
        }
    }

    fn finish_tab_drag(&mut self, cx: &mut Context<Self>) {
        match self.drag_state.clone() {
            DragState::Over {
                source_tab_id,
                target_tab_id,
            } => {
                let source_index = self.tab_bar.get_tab_index_by_id(source_tab_id);
                let target_index = self.tab_bar.get_tab_index_by_id(target_tab_id);
                let mut order_changed = false;
                if let (Some(source_index), Some(target_index)) = (source_index, target_index)
                    && source_index != target_index
                {
                    self.tab_bar.move_tab(source_index, target_index);
                    order_changed = true;
                }
                if order_changed {
                    self.persist_open_tabs();
                }
                self.drag_state = DragState::None;
                self.drag_mouse_position = None;
                cx.notify();
            }
            DragState::Started { .. } => {
                self.drag_state = DragState::None;
                self.drag_mouse_position = None;
                cx.notify();
            }
            DragState::None => {}
        }
    }

    fn render_drag_tab_preview(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let source_tab_id = self.current_drag_source_tab_id()?;
        let position = self.drag_mouse_position?;
        let tab = self
            .tab_bar
            .tabs()
            .iter()
            .find(|tab| tab.id == source_tab_id)?;

        let x: f32 = position.x.into();
        let y: f32 = position.y.into();

        Some(
            div()
                .id("drag-tab-preview")
                .absolute()
                // Keep the pointer outside the preview hit area.
                .left(px(x + 6.0))
                .top(px(y + 6.0))
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().primary.opacity(0.65))
                .bg(cx.theme().secondary.opacity(0.65))
                .shadow_lg()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground.opacity(0.95))
                        .child(tab.file_name()),
                )
                .into_any_element(),
        )
    }

    pub(super) fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.tab_bar.tabs().to_vec();
        let active_tab_id = self.tab_bar.active_tab_id();
        let recent_files_with_positions = self.recent_files_with_positions(&self.recent_files);
        let tab_recent_popup_open = self.recent_popup_open_for(RecentPopupAnchor::TabAddButton);
        let recent_popup_list_scroll = self.recent_popup_list_scroll.clone();
        let i18n = self.i18n();

        // 检查是否有文件打开，如果有，则过滤掉空的 Home 标签
        let has_file_open = tabs.iter().any(|tab| tab.path.is_some());
        let tabs_to_show: Vec<_> = tabs
            .iter()
            .filter(|tab| {
                if has_file_open {
                    // 有文件打开时，只显示有文件的标签
                    tab.path.is_some()
                } else {
                    // 没有文件时，显示所有标签（包括 Home）
                    true
                }
            })
            .collect();

        // 计算拖动指示器位置（基于可见 tab 的索引）
        let insertion_indicator_pos = match &self.drag_state {
            DragState::Over { target_tab_id, .. } => {
                tabs_to_show.iter().position(|tab| tab.id == *target_tab_id)
            }
            _ => None,
        };

        div()
            .h(px(TAB_BAR_HEIGHT))
            .w_full()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .flex()
            .items_center()
            .px_3()
            .gap_1()
            .relative() // 使子元素可以使用绝对定位
            .child(
                div().flex_shrink_0().child(
                    Popover::new("new-tab-popover")
                        .anchor(Corner::TopLeft)
                        .appearance(false)
                        .overlay_closable(false)
                        .open(tab_recent_popup_open)
                        .trigger(
                            Button::new("new-tab")
                                .xsmall()
                                .ghost()
                                .icon(
                                    Icon::new(crate::icons::IconName::Plus)
                                        .size_4()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .on_hover({
                                    let viewer = cx.entity();
                                    move |hovered, _, cx| {
                                        let _ = viewer.update(cx, |this, cx| {
                                            this.set_recent_popup_trigger_hovered(
                                                RecentPopupAnchor::TabAddButton,
                                                *hovered,
                                                cx,
                                            );
                                        });
                                    }
                                }),
                        )
                        .content({
                            let viewer = cx.entity();
                            let recent_files_with_positions = recent_files_with_positions.clone();
                            let i18n = i18n;
                            move |_, _window, cx| {
                                Self::render_recent_files_popup_panel(
                                    "new-tab-popup",
                                    1,
                                    i18n,
                                    viewer.clone(),
                                    recent_files_with_positions.clone(),
                                    &recent_popup_list_scroll,
                                    cx,
                                )
                            }
                        }),
                ),
            )
            .child(
                h_flex()
                    .id("tab-scroll")
                    .h_full()
                    .flex_1()
                    .overflow_x_scroll()
                    .track_scroll(&self.tab_bar_scroll)
                    .items_center()
                    .gap_1()
                    .children({
                        let mut elements = Vec::new();

                        for (index, tab) in tabs_to_show.iter().enumerate() {
                            let tab_id = tab.id;
                            let is_active = active_tab_id == Some(tab_id);
                            let is_drag_source = matches!(
                                self.drag_state,
                                DragState::Started { source_tab_id, .. } if source_tab_id == tab_id
                            ) || matches!(
                                self.drag_state,
                                DragState::Over { source_tab_id, .. } if source_tab_id == tab_id
                            );
                            let show_close_button = is_active || self.hovered_tab_id == Some(tab_id);
                            let close_icon_color = if show_close_button {
                                cx.theme().muted_foreground
                            } else {
                                cx.theme().muted_foreground.opacity(0.0)
                            };
                            let file_name = tab.file_name();
                            let is_home = tab.path.is_none();

                            // 在目标位置前插入垂直线指示器
                            if insertion_indicator_pos == Some(index) {
                                elements.push(
                                    div()
                                        .id(("indicator", index))
                                        .w_px()
                                        .flex_shrink_0()
                                        .h_6()
                                        .rounded_sm()
                                        .bg(cx.theme().primary)
                                        .into_any_element(),
                                );
                            }

                            elements.push(
                                div()
                                    .id(("tab", tab_id))
                                    .h(px(28.))
                                    .px_2()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .rounded_md()
                                    .bg(cx.theme().secondary)
                                    .when(is_active, |this| this.bg(cx.theme().background))
                                    .when(!is_active, |this| {
                                        this.hover(|this| this.bg(cx.theme().secondary.opacity(0.85)))
                                    })
                                    .on_hover({
                                        let viewer = cx.entity();
                                        move |hovered, _, cx| {
                                            let _ = viewer.update(cx, |this, cx| {
                                                if *hovered {
                                                    if this.hovered_tab_id != Some(tab_id) {
                                                        this.hovered_tab_id = Some(tab_id);
                                                        cx.notify();
                                                    }
                                                } else if this.hovered_tab_id == Some(tab_id) {
                                                    this.hovered_tab_id = None;
                                                    if let DragState::Over {
                                                        source_tab_id,
                                                        target_tab_id,
                                                    } = this.drag_state.clone()
                                                        && target_tab_id == tab_id
                                                    {
                                                        this.drag_state = DragState::Started {
                                                            source_tab_id,
                                                        };
                                                    }
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    })
                                    .on_mouse_move(cx.listener(
                                        move |this, event: &MouseMoveEvent, _, cx| {
                                            this.update_drag_mouse_position(event.position, cx);
                                            if let Some(source_tab_id) = this.current_drag_source_tab_id()
                                                && tab_id != source_tab_id
                                                && !matches!(this.drag_state, DragState::Over { source_tab_id: current_source, target_tab_id: current_target } if current_source == source_tab_id && current_target == tab_id)
                                            {
                                                this.drag_state = DragState::Over {
                                                    source_tab_id,
                                                    target_tab_id: tab_id,
                                                };
                                                cx.notify();
                                            }
                                        },
                                    ))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                            if this.tab_bar.get_tab_index_by_id(tab_id).is_some() {
                                                this.drag_state = DragState::Started {
                                                    source_tab_id: tab_id,
                                                };
                                                this.drag_mouse_position = Some(event.position);
                                                cx.notify();
                                            }
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                            this.open_tab_context_menu(tab_id, event.position, cx);
                                        }),
                                    )
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.finish_tab_drag(cx);
                                        }),
                                    )
                                    .when(matches!(self.drag_state, DragState::Started { source_tab_id, .. } if source_tab_id == tab_id)
                                        || matches!(self.drag_state, DragState::Over { source_tab_id, .. } if source_tab_id == tab_id), |div| {
                                        // 如果这个标签正在被拖动，给它特殊的视觉样式
                                        div.border_1()
                                            .border_color(cx.theme().primary)
                                            .bg(cx.theme().selection)
                                            .shadow_lg()
                                    })
                                    .child(
                                        div()
                                            .text_sm()
                                            .whitespace_nowrap()
                                            .text_color(if is_active
                                                || matches!(self.drag_state, DragState::Started { source_tab_id, .. } if source_tab_id == tab_id)
                                                || matches!(self.drag_state, DragState::Over { source_tab_id, .. } if source_tab_id == tab_id) {
                                                cx.theme().foreground
                                            } else {
                                                cx.theme().muted_foreground
                                            })
                                            .child(file_name.clone())
                                            .when(is_home, |this| {
                                                this.text_color(if matches!(self.drag_state, DragState::Started { source_tab_id, .. } if source_tab_id == tab_id)
                                                    || matches!(self.drag_state, DragState::Over { source_tab_id, .. } if source_tab_id == tab_id) {
                                                    cx.theme().foreground
                                                } else {
                                                    cx.theme().muted_foreground.opacity(0.6)
                                                })
                                            }),
                                    )
                                    .child(
                                        Button::new(("close-tab", tab_id))
                                            .xsmall()
                                            .ghost()
                                            .icon(
                                                Icon::new(crate::icons::IconName::WindowClose)
                                                    .size_3()
                                                    .text_color(close_icon_color),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.close_tab(tab_id, cx);
                                            })),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !is_active {
                                            this.switch_to_tab(tab_id, cx);
                                        }
                                    }))
                                    .when(is_drag_source, |this| this.cursor_grab())
                                    .when(!is_drag_source, |this| this.cursor_pointer())
                                    .into_any_element(),
                            );
                        }

                        // 处理拖动到末尾的情况
                        if insertion_indicator_pos == Some(tabs_to_show.len()) {
                            elements.push(
                                div()
                                    .id(("indicator", tabs_to_show.len()))
                                    .w_px()
                                    .flex_shrink_0()
                                    .h_6()
                                    .rounded_sm()
                                    .bg(cx.theme().primary)
                                    .into_any_element(),
                            );
                        }

                        elements
                    }),
            )
    }
}

impl Focusable for PdfViewer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PdfViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_initial_focus {
            self.needs_initial_focus = false;
            cx.focus_self(window);
        }
        if self.command_panel_open && self.command_panel_needs_focus {
            self.command_panel_needs_focus = false;
            let _ = self
                .command_panel_input_state
                .update(cx, |input, cx| input.focus(window, cx));
        }
        if !self.command_panel_open && self.needs_root_refocus {
            self.needs_root_refocus = false;
            window.focus(&self.focus_handle);
        }

        window.set_rem_size(cx.theme().font_size);

        let bounds = window.bounds();
        let current_size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let mut window_size_changed = false;
        if self.last_window_size != Some(current_size) {
            self.last_window_size = Some(current_size);
            if !window.is_maximized() && !window.is_fullscreen() {
                self.save_window_size(current_size.0, current_size.1);
            }
            window_size_changed = true;
        }

        let (
            page_count,
            current_page_num,
            zoom,
            _file_name,
            thumbnail_sizes,
            display_sizes,
            _display_base_width,
            display_panel_width,
        ) = {
            let active_tab = self.active_tab();
            let page_count = active_tab.map(|t| t.pages.len()).unwrap_or(0);
            let current_page_num = if page_count == 0 {
                0
            } else {
                active_tab.map(|t| t.active_page + 1).unwrap_or(0)
            };
            let zoom = active_tab.map(|t| t.zoom).unwrap_or(1.0);

            let file_name = active_tab
                .and_then(|t| t.path.as_ref())
                .map(|p| display_file_name(p))
                .unwrap_or_else(|| self.i18n().file_not_opened().to_string());

            let display_base_width = active_tab
                .map(|t| self.display_base_width(window, t.zoom))
                .unwrap_or(DISPLAY_MIN_WIDTH);
            let display_panel_width = active_tab
                .map(|t| self.display_panel_width(window, t.zoom))
                .unwrap_or(DISPLAY_MIN_WIDTH);

            let thumbnail_sizes = active_tab
                .map(|t| self.thumbnail_item_sizes(&t.pages))
                .unwrap_or_else(|| Rc::new(Vec::new()));
            let display_sizes = active_tab
                .map(|t| self.display_item_sizes(&t.pages, display_base_width))
                .unwrap_or_else(|| Rc::new(Vec::new()));

            (
                page_count,
                current_page_num,
                zoom,
                file_name,
                thumbnail_sizes,
                display_sizes,
                display_base_width,
                display_panel_width,
            )
        };

        let zoom_label: SharedString = format!("{:.0}%", zoom * 100.0).into();

        // 更新当前标签页的显示滚动偏移
        let target_width = if let Some(tab) = self.active_tab() {
            self.display_target_width(window, tab.zoom)
        } else {
            220
        };
        let mut display_layout_changed = false;
        let mut page_to_restore_after_layout_change = None;
        if let Some(tab) = self.active_tab_mut() {
            let target_width_changed = tab.last_display_target_width != target_width;
            display_layout_changed = window_size_changed || target_width_changed;

            if display_layout_changed {
                // Invalidate pending scroll-to-page sync jobs from previous layout.
                tab.display_scroll_sync_epoch = tab.display_scroll_sync_epoch.wrapping_add(1);
            }

            if target_width_changed {
                tab.last_display_target_width = target_width;
            }

            if display_layout_changed {
                if tab.pages.is_empty() {
                    tab.last_display_scroll_offset = Some(tab.display_scroll.offset());
                } else {
                    // Keep the current page stable across resize/maximize/restore.
                    let keep_page = tab.active_page.min(tab.pages.len().saturating_sub(1));
                    tab.active_page = keep_page;
                    tab.selected_page = keep_page;
                    tab.last_display_visible_range =
                        Some(keep_page..keep_page.saturating_add(1).min(tab.pages.len()));
                    tab.last_display_scroll_offset = Some(tab.display_scroll.offset());
                    page_to_restore_after_layout_change = Some(keep_page);
                }
            }
        }
        if let Some(keep_page) = page_to_restore_after_layout_change {
            self.schedule_restore_current_page_after_layout_change(keep_page, cx);
        }
        if !display_layout_changed {
            self.on_display_scroll_offset_changed(cx);
        }

        let context_menu = self.render_context_menu(cx);
        let drag_tab_preview = self.render_drag_tab_preview(cx);
        let command_panel = self.render_command_panel(cx);
        let about_dialog = self.render_about_dialog(cx);
        let settings_dialog = self.render_settings_dialog(cx);

        div()
            .size_full()
            .on_action(cx.listener(|this, _: &ShowAboutMenu, _, cx| {
                this.open_about_dialog(cx);
            }))
            .on_action(cx.listener(|this, _: &CheckForUpdatesMenu, _, cx| {
                this.open_about_dialog(cx);
                this.check_for_updates(cx);
            }))
            .on_action(cx.listener(|this, _: &ShowSettingsMenu, _, cx| {
                this.open_settings_dialog(cx);
            }))
            .on_action(cx.listener(|this, _: &EnableLoggingMenu, _, cx| {
                if crate::logger::enable_file_logging() {
                    configure_app_menus(cx, this.i18n());
                }
            }))
            .on_action(cx.listener(|this, _: &DisableLoggingMenu, _, cx| {
                crate::logger::disable_file_logging();
                configure_app_menus(cx, this.i18n());
            }))
            .on_action(cx.listener(|this, _: &OpenLogsMenu, _, _| {
                this.open_logs_directory();
            }))
            .child(
                div()
                    .v_flex()
                    .size_full()
                    .bg(cx.theme().background)
                    .relative()
                    .track_focus(&self.focus_handle)
                    .capture_key_down(cx.listener(
                        |this, event: &gpui::KeyDownEvent, window, cx| {
                            this.handle_key_down(event, window, cx);
                        },
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.close_context_menu(cx);
                            this.close_bookmark_popup(cx);
                            window.focus(&this.focus_handle);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_drag_mouse_position(event.position, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.finish_tab_drag(cx);
                        }),
                    )
                    .child(
                        div()
                            .id("title-bar")
                            .w_full()
                            .v_flex()
                            .bg(cx.theme().title_bar)
                            .child(
                                div()
                                    .id("title-bar-top")
                                    .h(px(TITLE_BAR_HEIGHT))
                                    .w_full()
                                    .relative()
                                    .border_b_1()
                                    .border_color(cx.theme().title_bar_border)
                                    .when(cfg!(target_os = "macos"), |this| {
                                        this.child(
                                            div()
                                                .id("title-drag-area")
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .right_0()
                                                .bottom_0()
                                                .on_double_click(|_, window, _| {
                                                    window.titlebar_double_click()
                                                })
                                                .window_control_area(WindowControlArea::Drag),
                                        )
                                    })
                                    .child(
                                        div()
                                            .id("title-bar-foreground")
                                            .h_full()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .id("title-nav-host")
                                                    .h_full()
                                                    .flex_1()
                                                    .pl(px(TITLE_BAR_CONTENT_LEFT_PADDING))
                                                    .pr_1()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(self.render_menu_bar(
                                                        page_count,
                                                        current_page_num,
                                                        zoom_label,
                                                        self.titlebar_preferences.show_navigation,
                                                        self.titlebar_preferences.show_zoom,
                                                        cx,
                                                    )),
                                            )
                                            .when(!cfg!(target_os = "macos"), |this| {
                                                this.child(
                                                    div()
                                                        .id("title-drag-area")
                                                        .h_full()
                                                        .w(px(24.))
                                                        .flex_shrink_0()
                                                        .map(|this| {
                                                            let should_move =
                                                                Rc::new(Cell::new(false));
                                                            this.on_double_click(
                                                                |_, window, _| window.zoom_window(),
                                                            )
                                                            .on_mouse_down(MouseButton::Left, {
                                                                let should_move =
                                                                    should_move.clone();
                                                                move |_, _, _| {
                                                                    should_move.set(true);
                                                                }
                                                            })
                                                            .on_mouse_down_out({
                                                                let should_move =
                                                                    should_move.clone();
                                                                move |_, _, _| {
                                                                    should_move.set(false);
                                                                }
                                                            })
                                                            .on_mouse_up(MouseButton::Left, {
                                                                let should_move =
                                                                    should_move.clone();
                                                                move |_, _, _| {
                                                                    should_move.set(false);
                                                                }
                                                            })
                                                            .on_mouse_move({
                                                                let should_move =
                                                                    should_move.clone();
                                                                move |_, window, _| {
                                                                    if should_move.get() {
                                                                        should_move.set(false);
                                                                        window.start_window_move();
                                                                    }
                                                                }
                                                            })
                                                            .window_control_area(
                                                                WindowControlArea::Drag,
                                                            )
                                                        }),
                                                )
                                            })
                                            .when(!cfg!(target_os = "macos"), |this| {
                                                this.child(
                                                    div()
                                                        .h_full()
                                                        .pr_1()
                                                        .flex()
                                                        .items_center()
                                                        .gap_1()
                                                        .child(
                                                            Button::new("window-minimize")
                                                                .ghost()
                                                                .small()
                                                                .icon(
                                                                    Icon::new(
                                                                        crate::icons::IconName::WindowMinimize,
                                                                    )
                                                                    .text_color(cx.theme().foreground),
                                                                )
                                                                .on_click(|_, window, _| {
                                                                    window.minimize_window()
                                                                }),
                                                        )
                                                        .child(
                                                            Button::new("window-maximize")
                                                                .ghost()
                                                                .small()
                                                                .icon(
                                                                    Icon::new(if window.is_maximized() {
                                                                        crate::icons::IconName::WindowRestore
                                                                    } else {
                                                                        crate::icons::IconName::WindowMaximize
                                                                    })
                                                                    .text_color(cx.theme().foreground),
                                                                )
                                                                .on_click(|_, window, _| {
                                                                    zoom_or_restore_window(window)
                                                                }),
                                                        )
                                                        .child(
                                                            Button::new("window-close")
                                                                .ghost()
                                                                .small()
                                                                .icon(
                                                                    Icon::new(crate::icons::IconName::WindowClose)
                                                                        .text_color(cx.theme().foreground),
                                                                )
                                                                .on_click(|_, window, _| {
                                                                    window.remove_window()
                                                                }),
                                                        ),
                                                )
                                            }),
                                    )
                            )
                            .child(self.render_tab_bar(cx))
                    )
                    .child(
                        div()
                            .h_full()
                            .w_full()
                            .flex()
                            .overflow_hidden()
                            .when(self.show_thumbnail_panel(), |this| {
                                this.child(self.render_thumbnail_panel(
                                    page_count,
                                    thumbnail_sizes,
                                    cx,
                                ))
                            })
                            .child(self.render_display_panel(
                                page_count,
                                display_sizes,
                                display_panel_width,
                                cx,
                            )),
                    )
                    .when(context_menu.is_some(), |this| {
                        this.child(context_menu.unwrap())
                    })
                    .when(drag_tab_preview.is_some(), |this| {
                        this.child(drag_tab_preview.unwrap())
                    })
                    .when(command_panel.is_some(), |this| {
                        this.child(command_panel.unwrap())
                    })
                    .when(about_dialog.is_some(), |this| {
                        this.child(about_dialog.unwrap())
                    })
                    .when(settings_dialog.is_some(), |this| {
                        this.child(settings_dialog.unwrap())
                    }),
            )
    }
}
