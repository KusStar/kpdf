#[path = "display_list.rs"]
mod display_list;
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
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{button::*, *};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use std::time::Duration;

use self::tab::{PdfTab, TabBar};
use self::text_selection::copy_to_clipboard;
use self::utils::{display_file_name, load_display_images, load_document_summary};

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
const RECENT_POPUP_CLOSE_DELAY_MS: u64 = 120;
const RECENT_FILES_TREE: &str = "recent_files";
const FILE_POSITIONS_TREE: &str = "file_positions";
const WINDOW_SIZE_TREE: &str = "window_size";
const WINDOW_SIZE_KEY_WIDTH: &str = "width";
const WINDOW_SIZE_KEY_HEIGHT: &str = "height";
const TAB_BAR_HEIGHT: f32 = 36.0;
#[cfg(target_os = "macos")]
const TITLE_BAR_CONTENT_LEFT_PADDING: f32 = 80.0;
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_CONTENT_LEFT_PADDING: f32 = 12.0;

pub use self::utils::PageSummary;

pub struct PdfViewer {
    language: Language,
    tab_bar: TabBar,
    recent_store: Option<sled::Tree>,
    position_store: Option<sled::Tree>,
    window_size_store: Option<sled::Tree>,
    last_window_size: Option<(f32, f32)>,
    recent_files: Vec<PathBuf>,
    recent_popup_open: bool,
    recent_popup_trigger_hovered: bool,
    recent_popup_panel_hovered: bool,
    recent_popup_hover_epoch: u64,
    context_menu_open: bool,
    context_menu_position: Option<Point<Pixels>>,
    #[allow(dead_code)]
    context_menu_tab_id: Option<usize>,
}

impl PdfViewer {
    pub fn new() -> Self {
        let language = Language::detect();
        let (recent_store, position_store, window_size_store) = Self::open_persistent_stores();
        let recent_files = recent_store
            .as_ref()
            .map(Self::load_recent_files_from_store)
            .unwrap_or_default();

        let mut tab_bar = TabBar::new();
        // 创建第一个空标签页
        tab_bar.create_tab();

        Self {
            language,
            tab_bar,
            recent_store,
            position_store,
            window_size_store,
            last_window_size: None,
            recent_files,
            recent_popup_open: false,
            recent_popup_trigger_hovered: false,
            recent_popup_panel_hovered: false,
            recent_popup_hover_epoch: 0,
            context_menu_open: false,
            context_menu_position: None,
            context_menu_tab_id: None,
        }
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

    fn open_persistent_stores() -> (Option<sled::Tree>, Option<sled::Tree>, Option<sled::Tree>) {
        let db_path = Self::recent_files_db_path();
        if let Some(parent) = db_path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                eprintln!("[store] create dir failed: {}", parent.to_string_lossy());
                return (None, None, None);
            }
        }

        let db = match sled::open(&db_path) {
            Ok(db) => db,
            Err(err) => {
                eprintln!(
                    "[store] open db failed: {} | {}",
                    db_path.to_string_lossy(),
                    err
                );
                return (None, None, None);
            }
        };

        let recent_store = match db.open_tree(RECENT_FILES_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                eprintln!("[store] open tree failed: {} | {}", RECENT_FILES_TREE, err);
                None
            }
        };
        let position_store = match db.open_tree(FILE_POSITIONS_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                eprintln!(
                    "[store] open tree failed: {} | {}",
                    FILE_POSITIONS_TREE, err
                );
                None
            }
        };
        let window_size_store = match db.open_tree(WINDOW_SIZE_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                eprintln!("[store] open tree failed: {} | {}", WINDOW_SIZE_TREE, err);
                None
            }
        };

        eprintln!(
            "[store] init recent={} positions={} window_size={} path={}",
            recent_store.is_some(),
            position_store.is_some(),
            window_size_store.is_some(),
            db_path.to_string_lossy()
        );

        (recent_store, position_store, window_size_store)
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

    fn open_pdf_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.close_recent_popup(cx);

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
                self.tab_bar.switch_to_tab(tab.id);
                cx.notify();
                return;
            }
        }

        // 在新标签页打开
        self.open_pdf_path_in_new_tab(path, cx);
    }

    fn open_pdf_path_in_current_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let language = self.language;

        // 先重置当前标签页
        if let Some(tab) = self.active_tab_mut() {
            tab.reset_page_render_state();
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

            let _ = view.update(cx, |this, cx| match parsed {
                Ok(mut pages) => {
                    pages.sort_by_key(|p| p.index);

                    let restored_page = this.load_saved_file_position(&path);

                    if let Some(tab) = this.active_tab_mut() {
                        tab.path = Some(path.clone());
                        tab.pages = pages;
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
                    }

                    this.remember_recent_file(&path);
                    cx.notify();
                }
                Err(_) => {
                    if let Some(tab) = this.active_tab_mut() {
                        tab.path = Some(path.clone());
                        tab.pages.clear();
                        tab.selected_page = 0;
                        tab.active_page = 0;
                        tab.reset_page_render_state();
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn open_pdf_path_in_new_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let language = self.language;

        cx.spawn(async move |view, cx| {
            let parsed = cx
                .background_executor()
                .spawn({
                    let path = path.clone();
                    async move { load_document_summary(&path, language) }
                })
                .await;

            let _ = view.update(cx, |this, cx| match parsed {
                Ok(mut pages) => {
                    pages.sort_by_key(|p| p.index);
                    let tab_id = this.tab_bar.create_tab_with_path(path.clone(), pages);
                    this.tab_bar.switch_to_tab(tab_id);

                    let restored_page = this.load_saved_file_position(&path);

                    if let Some(tab) = this.tab_bar.get_active_tab_mut() {
                        let initial_page = restored_page
                            .unwrap_or(0)
                            .min(tab.pages.len().saturating_sub(1));
                        tab.selected_page = initial_page;
                        tab.active_page = initial_page;
                        tab.zoom = 1.0;

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
                    }

                    this.remember_recent_file(&path);
                    cx.notify();
                }
                Err(_) => {
                    let tab_id = this.tab_bar.create_tab_with_path(path.clone(), Vec::new());
                    this.tab_bar.switch_to_tab(tab_id);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn create_new_tab(&mut self, cx: &mut Context<Self>) {
        self.tab_bar.create_tab();
        cx.notify();
    }

    fn close_tab(&mut self, tab_id: usize, cx: &mut Context<Self>) {
        // 先持久化要关闭的标签页的位置
        if let Some(tab) = self.tab_bar.tabs().iter().find(|t| t.id == tab_id) {
            if let Some(ref path) = tab.path {
                if !tab.pages.is_empty() {
                    let page_index = tab.active_page.min(tab.pages.len().saturating_sub(1));
                    self.save_file_position(path, page_index);
                }
            }
        }

        self.tab_bar.close_tab(tab_id);

        // 如果没有标签页了，创建一个空的
        if !self.tab_bar.has_tabs() {
            self.tab_bar.create_tab();
        }

        cx.notify();
    }

    fn switch_to_tab(&mut self, tab_id: usize, cx: &mut Context<Self>) {
        if self.tab_bar.switch_to_tab(tab_id) {
            cx.notify();
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
            eprintln!("[window_size] save width failed");
        }
        if store
            .insert(WINDOW_SIZE_KEY_HEIGHT, height_bytes.as_slice())
            .is_err()
        {
            eprintln!("[window_size] save height failed");
        }
        let _ = store.flush();
    }

    fn recent_popup_open(&self) -> bool {
        self.recent_popup_open
    }

    fn set_recent_popup_trigger_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.recent_popup_trigger_hovered != hovered {
            self.recent_popup_trigger_hovered = hovered;
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
        if self.recent_popup_trigger_hovered || self.recent_popup_panel_hovered {
            self.recent_popup_hover_epoch = self.recent_popup_hover_epoch.wrapping_add(1);
            if !self.recent_popup_open {
                self.recent_popup_open = true;
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
                if this.recent_popup_trigger_hovered || this.recent_popup_panel_hovered {
                    return;
                }
                if this.recent_popup_open {
                    this.recent_popup_open = false;
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
        if self.recent_popup_panel_hovered {
            self.recent_popup_panel_hovered = false;
            has_changed = true;
        }
        if self.recent_popup_open {
            self.recent_popup_open = false;
            has_changed = true;
        }
        if has_changed {
            cx.notify();
        }
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
        (viewport_width - SIDEBAR_WIDTH).max(DISPLAY_MIN_WIDTH)
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
                        eprintln!("[copy] failed to copy to clipboard: {}", err);
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
        cx.notify();
    }

    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        self.context_menu_open = false;
        self.context_menu_position = None;
        cx.notify();
    }

    pub fn has_text_selection(&self) -> bool {
        self.active_tab()
            .and_then(|tab| tab.text_selection_manager.borrow().get_selected_text())
            .is_some()
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

    pub(super) fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.tab_bar.tabs().to_vec();
        let active_tab_id = self.tab_bar.active_tab_id();

        div()
            .h(px(TAB_BAR_HEIGHT))
            .w_full()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .flex()
            .items_center()
            .px_1()
            .gap_1()
            .overflow_x_hidden()
            .children(tabs.iter().map(|tab| {
                let tab_id = tab.id;
                let is_active = active_tab_id == Some(tab_id);
                let file_name = tab.file_name();
                let is_untitled = tab.path.is_none();

                div()
                    .id(("tab", tab_id))
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_active, |this| this.bg(cx.theme().secondary_active))
                    .when(!is_active, |this| {
                        this.hover(|this| this.bg(cx.theme().secondary_hover))
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(if is_active {
                                cx.theme().secondary_foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(file_name.clone())
                            .when(is_untitled, |this| {
                                this.text_color(cx.theme().muted_foreground.opacity(0.6))
                            }),
                    )
                    .child(
                        Button::new(("close-tab", tab_id))
                            .xsmall()
                            .ghost()
                            .icon(
                                Icon::new(IconName::WindowClose)
                                    .size_3()
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
                    .into_any_element()
            }))
            .child(
                Button::new("new-tab")
                    .xsmall()
                    .ghost()
                    .icon(
                        Icon::new(IconName::Plus)
                            .size_4()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.create_new_tab(cx);
                    })),
            )
    }
}

impl Render for PdfViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(cx.theme().font_size);

        let bounds = window.bounds();
        let current_size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        if self.last_window_size != Some(current_size) {
            self.last_window_size = Some(current_size);
            self.save_window_size(current_size.0, current_size.1);
        }

        let (
            page_count,
            current_page_num,
            zoom,
            file_name,
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

        let recent_popup_open = self.recent_popup_open();
        let recent_files = self.recent_files.clone();
        let zoom_label: SharedString = format!("{:.0}%", zoom * 100.0).into();

        // 更新当前标签页的显示滚动偏移
        let target_width = if let Some(tab) = self.active_tab() {
            self.display_target_width(window, tab.zoom)
        } else {
            220
        };
        if let Some(tab) = self.active_tab_mut() {
            tab.last_display_target_width = target_width;
        }
        self.on_display_scroll_offset_changed(cx);

        let context_menu = self.render_context_menu(cx);

        window_border().child(
            div()
                .v_flex()
                .size_full()
                .bg(cx.theme().background)
                .relative()
                .child(
                    div()
                        .h(px(34.))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(cx.theme().title_bar_border)
                        .bg(cx.theme().title_bar)
                        .child(
                            div()
                                .h_full()
                                .flex_1()
                                .pl(px(TITLE_BAR_CONTENT_LEFT_PADDING))
                                .pr_3()
                                .flex()
                                .items_center()
                                .gap_2()
                                .window_control_area(WindowControlArea::Drag)
                                .child(
                                    Icon::new(IconName::File)
                                        .size_4()
                                        .text_color(cx.theme().foreground),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(cx.theme().foreground)
                                        .child("kPDF"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(file_name),
                                ),
                        )
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
                                                Icon::new(IconName::WindowMinimize)
                                                    .text_color(cx.theme().foreground),
                                            )
                                            .on_click(|_, window, _| window.minimize_window()),
                                    )
                                    .child(
                                        Button::new("window-maximize")
                                            .ghost()
                                            .small()
                                            .icon(
                                                Icon::new(if window.is_maximized() {
                                                    IconName::WindowRestore
                                                } else {
                                                    IconName::WindowMaximize
                                                })
                                                .text_color(cx.theme().foreground),
                                            )
                                            .on_click(|_, window, _| window.zoom_window()),
                                    )
                                    .child(
                                        Button::new("window-close")
                                            .ghost()
                                            .small()
                                            .icon(
                                                Icon::new(IconName::WindowClose)
                                                    .text_color(cx.theme().foreground),
                                            )
                                            .on_click(|_, window, _| window.remove_window()),
                                    ),
                            )
                        }),
                )
                .child(self.render_tab_bar(cx))
                .child(self.render_menu_bar(
                    page_count,
                    current_page_num,
                    recent_popup_open,
                    recent_files,
                    zoom_label,
                    cx,
                ))
                .child(
                    div()
                        .h_full()
                        .w_full()
                        .flex()
                        .overflow_hidden()
                        .child(self.render_thumbnail_panel(page_count, thumbnail_sizes, cx))
                        .child(self.render_display_panel(
                            page_count,
                            display_sizes,
                            display_panel_width,
                            cx,
                        ))
                        .on_key_down(cx.listener(
                            |this, event: &gpui::KeyDownEvent, _window, cx| {
                                // Handle Cmd+C / Ctrl+C for copy
                                if event.keystroke.key == "c" && event.keystroke.modifiers.platform
                                {
                                    this.copy_selected_text();
                                    cx.stop_propagation();
                                }
                                // Handle Cmd+A / Ctrl+A for select all on current page
                                else if event.keystroke.key == "a"
                                    && event.keystroke.modifiers.platform
                                {
                                    this.select_all_text(cx);
                                    cx.stop_propagation();
                                }
                                // Handle Escape to clear selection
                                else if event.keystroke.key == "escape" {
                                    this.clear_text_selection(cx);
                                    cx.stop_propagation();
                                }
                                // Handle Cmd+W / Ctrl+W to close current tab
                                else if event.keystroke.key == "w"
                                    && event.keystroke.modifiers.platform
                                {
                                    this.close_current_tab(cx);
                                    cx.stop_propagation();
                                }
                            },
                        )),
                )
                .when(context_menu.is_some(), |this| {
                    this.child(context_menu.unwrap())
                }),
        )
    }
}
