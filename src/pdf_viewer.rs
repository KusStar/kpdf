#[path = "display_list.rs"]
mod display_list;
#[path = "menu_bar.rs"]
mod menu_bar;
#[path = "thumbnail_list.rs"]
mod thumbnail_list;
#[path = "utils.rs"]
mod utils;

use crate::i18n::{I18n, Language};
use gpui::*;
use gpui::prelude::FluentBuilder as _;
use gpui_component::{button::*, *};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

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
#[cfg(target_os = "macos")]
const TITLE_BAR_CONTENT_LEFT_PADDING: f32 = 80.0;
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_CONTENT_LEFT_PADDING: f32 = 12.0;

use self::utils::{display_file_name, load_display_images, load_document_summary};

#[derive(Clone)]
struct PageSummary {
    index: usize,
    width_pt: f32,
    height_pt: f32,
    thumbnail_image: Option<Arc<RenderImage>>,
    thumbnail_render_width: u32,
    thumbnail_failed: bool,
    display_image: Option<Arc<RenderImage>>,
    display_render_width: u32,
    display_failed: bool,
}

pub struct PdfViewer {
    language: Language,
    path: Option<PathBuf>,
    pages: Vec<PageSummary>,
    selected_page: usize,
    active_page: usize,
    recent_store: Option<sled::Tree>,
    position_store: Option<sled::Tree>,
    recent_files: Vec<PathBuf>,
    recent_popup_open: bool,
    recent_popup_trigger_hovered: bool,
    recent_popup_panel_hovered: bool,
    recent_popup_hover_epoch: u64,
    zoom: f32,
    thumbnail_scroll: VirtualListScrollHandle,
    display_scroll: VirtualListScrollHandle,
    thumbnail_loading: HashSet<usize>,
    thumbnail_inflight_tasks: usize,
    thumbnail_epoch: u64,
    last_thumbnail_visible_range: Option<std::ops::Range<usize>>,
    display_loading: HashSet<usize>,
    display_inflight_tasks: usize,
    display_epoch: u64,
    last_display_visible_range: Option<std::ops::Range<usize>>,
    last_display_target_width: u32,
    display_scroll_sync_epoch: u64,
    last_display_scroll_offset: Option<Point<Pixels>>,
    suppress_display_scroll_sync_once: bool,
    last_saved_position: Option<(PathBuf, usize)>,
}

impl PdfViewer {
    pub fn new() -> Self {
        let language = Language::detect();
        let (recent_store, position_store) = Self::open_persistent_stores();
        let recent_files = recent_store
            .as_ref()
            .map(Self::load_recent_files_from_store)
            .unwrap_or_default();

        Self {
            language,
            path: None,
            pages: Vec::new(),
            selected_page: 0,
            active_page: 0,
            recent_store,
            position_store,
            recent_files,
            recent_popup_open: false,
            recent_popup_trigger_hovered: false,
            recent_popup_panel_hovered: false,
            recent_popup_hover_epoch: 0,
            zoom: 1.0,
            thumbnail_scroll: VirtualListScrollHandle::new(),
            display_scroll: VirtualListScrollHandle::new(),
            thumbnail_loading: HashSet::new(),
            thumbnail_inflight_tasks: 0,
            thumbnail_epoch: 0,
            last_thumbnail_visible_range: None,
            display_loading: HashSet::new(),
            display_inflight_tasks: 0,
            display_epoch: 0,
            last_display_visible_range: None,
            last_display_target_width: DISPLAY_MIN_WIDTH as u32,
            display_scroll_sync_epoch: 0,
            last_display_scroll_offset: None,
            suppress_display_scroll_sync_once: false,
            last_saved_position: None,
        }
    }

    fn i18n(&self) -> I18n {
        I18n::new(self.language)
    }

    fn reset_thumbnail_render_state(&mut self) {
        self.thumbnail_loading.clear();
        self.thumbnail_inflight_tasks = 0;
        self.thumbnail_epoch = self.thumbnail_epoch.wrapping_add(1);
        self.last_thumbnail_visible_range = None;
    }

    fn reset_display_render_state(&mut self) {
        self.display_loading.clear();
        self.display_inflight_tasks = 0;
        self.display_epoch = self.display_epoch.wrapping_add(1);
        self.last_display_visible_range = None;
    }

    fn reset_page_render_state(&mut self) {
        self.reset_thumbnail_render_state();
        self.reset_display_render_state();
    }

    fn open_persistent_stores() -> (Option<sled::Tree>, Option<sled::Tree>) {
        let db_path = Self::recent_files_db_path();
        if let Some(parent) = db_path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                eprintln!("[store] create dir failed: {}", parent.to_string_lossy());
                return (None, None);
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
                return (None, None);
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

        eprintln!(
            "[store] init recent={} positions={} path={}",
            recent_store.is_some(),
            position_store.is_some(),
            db_path.to_string_lossy()
        );

        (recent_store, position_store)
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

    fn open_pdf_dialog(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.close_recent_popup(cx);

        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.i18n().open_pdf_prompt().into()),
        });

        cx.spawn(async move |view, cx| {
            let picker_result = picker.await;
            match picker_result {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next() {
                        let _ = view.update(cx, |this, cx| {
                            this.open_pdf_path(path, cx);
                        });
                    } else {
                        let _ = view.update(cx, |this, cx| {
                            this.reset_page_render_state();
                            cx.notify();
                        });
                    }
                }
                Ok(Ok(None)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.reset_page_render_state();
                        cx.notify();
                    });
                }
                Ok(Err(err)) => {
                    let _ = view.update(cx, |this, cx| {
                        let _ = err;
                        this.reset_page_render_state();
                        cx.notify();
                    });
                }
                Err(err) => {
                    let _ = view.update(cx, |this, cx| {
                        let _ = err;
                        this.reset_page_render_state();
                        cx.notify();
                    });
                }
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

        self.open_pdf_path(path, cx);
    }

    fn open_pdf_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.reset_page_render_state();
        cx.notify();
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
                    this.path = Some(path.clone());
                    this.pages = pages;
                    let restored_page = this.load_saved_file_position(&path);
                    match restored_page {
                        Some(page_index) => {
                            eprintln!(
                                "[position] restore hit: {} -> page {}",
                                path.display(),
                                page_index + 1
                            );
                        }
                        None => {
                            eprintln!("[position] restore miss: {}", path.display());
                        }
                    }
                    let restored_page = restored_page.unwrap_or(0);
                    let initial_page = restored_page.min(this.pages.len().saturating_sub(1));
                    if initial_page != restored_page {
                        eprintln!(
                            "[position] restore clamp: {} requested={} available_max={}",
                            path.display(),
                            restored_page + 1,
                            initial_page + 1
                        );
                    }
                    this.selected_page = initial_page;
                    this.active_page = initial_page;
                    this.zoom = 1.0;
                    this.reset_page_render_state();
                    this.remember_recent_file(&path);
                    if !this.pages.is_empty() {
                        let strategy = if initial_page == 0 {
                            ScrollStrategy::Top
                        } else {
                            ScrollStrategy::Center
                        };
                        this.suppress_display_scroll_sync_once = true;
                        this.thumbnail_scroll.scroll_to_item(initial_page, strategy);
                        this.display_scroll.scroll_to_item(initial_page, strategy);
                    }
                    cx.notify();
                }
                Err(err) => {
                    this.path = Some(path.clone());
                    this.pages.clear();
                    this.selected_page = 0;
                    this.active_page = 0;
                    this.reset_page_render_state();
                    let _ = err;
                    cx.notify();
                }
            });
        })
        .detach();
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

    fn save_file_position(&mut self, path: &Path, page_index: usize) {
        let Some(store) = self.position_store.as_ref() else {
            eprintln!(
                "[position] save skipped (no store): {} -> page {}",
                path.display(),
                page_index + 1
            );
            return;
        };

        let page_bytes = (page_index as u64).to_be_bytes();
        match store.insert(Self::file_position_key(path), page_bytes.as_slice()) {
            Ok(_) => {
                self.last_saved_position = Some((path.to_path_buf(), page_index));
                match store.flush() {
                    Ok(_) => {
                        eprintln!(
                            "[position] save ok: {} -> page {}",
                            path.display(),
                            page_index + 1
                        );
                    }
                    Err(err) => {
                        eprintln!(
                            "[position] save flush failed: {} -> page {} | {}",
                            path.display(),
                            page_index + 1,
                            err
                        );
                    }
                }
            }
            Err(err) => {
                eprintln!(
                    "[position] save failed: {} -> page {} | {}",
                    path.display(),
                    page_index + 1,
                    err
                );
            }
        }
    }

    fn persist_current_file_position(&mut self) {
        if self.pages.is_empty() {
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };

        let page_index = self.active_page.min(self.pages.len().saturating_sub(1));
        if self
            .last_saved_position
            .as_ref()
            .map(|(saved_path, saved_index)| saved_path == &path && *saved_index == page_index)
            .unwrap_or(false)
        {
            return;
        }

        self.save_file_position(&path, page_index);
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
        if index < self.pages.len() {
            self.selected_page = index;
            self.active_page = index;
            self.sync_scroll_to_selected();
            self.persist_current_file_position();
            cx.notify();
        }
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.active_page > 0 {
            self.select_page(self.active_page - 1, cx);
        }
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        if self.active_page + 1 < self.pages.len() {
            self.select_page(self.active_page + 1, cx);
        }
    }

    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.zoom = (self.zoom + ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
        cx.notify();
    }

    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.zoom = (self.zoom - ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
        cx.notify();
    }

    fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        cx.notify();
    }

    fn sync_scroll_to_selected(&mut self) {
        self.suppress_display_scroll_sync_once = true;
        self.thumbnail_scroll
            .scroll_to_item(self.selected_page, ScrollStrategy::Center);
        self.display_scroll
            .scroll_to_item(self.selected_page, ScrollStrategy::Center);
    }

    fn schedule_thumbnail_sync_after_display_scroll(&mut self, cx: &mut Context<Self>) {
        self.display_scroll_sync_epoch = self.display_scroll_sync_epoch.wrapping_add(1);
        let sync_epoch = self.display_scroll_sync_epoch;

        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(DISPLAY_SCROLL_SYNC_DELAY_MS))
                .await;

            let _ = view.update(cx, |this, cx| {
                if this.display_scroll_sync_epoch != sync_epoch || this.pages.is_empty() {
                    return;
                }

                let next_active = this
                    .last_display_visible_range
                    .as_ref()
                    .map(|range| range.start.min(this.pages.len().saturating_sub(1)))
                    .unwrap_or_else(|| this.active_page.min(this.pages.len().saturating_sub(1)));
                if this.active_page != next_active {
                    this.active_page = next_active;
                    this.persist_current_file_position();
                }

                this.thumbnail_scroll
                    .scroll_to_item(next_active, ScrollStrategy::Center);
                cx.notify();
            });
        })
        .detach();
    }

    fn on_display_scroll_offset_changed(&mut self, cx: &mut Context<Self>) {
        let offset = self.display_scroll.offset();
        let has_changed = self
            .last_display_scroll_offset
            .map(|last| last != offset)
            .unwrap_or(false);
        self.last_display_scroll_offset = Some(offset);

        if has_changed && !self.pages.is_empty() {
            if self.suppress_display_scroll_sync_once {
                self.suppress_display_scroll_sync_once = false;
                return;
            }
            self.schedule_thumbnail_sync_after_display_scroll(cx);
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

    fn thumbnail_item_sizes(&self) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            self.pages
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
        if candidate_order.is_empty() || self.pages.is_empty() {
            return;
        }

        let Some(path) = self.path.clone() else {
            return;
        };

        if self.thumbnail_inflight_tasks >= THUMB_MAX_PARALLEL_TASKS {
            return;
        }

        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for ix in candidate_order {
            if !seen.insert(ix) {
                continue;
            }

            let Some(page) = self.pages.get(ix) else {
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
            self.thumbnail_loading.insert(*ix);
        }
        self.thumbnail_inflight_tasks = self.thumbnail_inflight_tasks.saturating_add(1);
        let language = self.language;

        let epoch = self.thumbnail_epoch;
        cx.spawn(async move |view, cx| {
            let load_result = cx
                .background_executor()
                .spawn(async move {
                    let loaded = load_display_images(&path, &pending, target_width, language);
                    (pending, target_width, loaded)
                })
                .await;

            let _ = view.update(cx, |this, cx| {
                this.thumbnail_inflight_tasks = this.thumbnail_inflight_tasks.saturating_sub(1);
                if this.thumbnail_epoch != epoch {
                    return;
                }

                let (requested_indices, loaded_target_width, loaded_result) = load_result;
                let mut loaded_indices = HashSet::new();

                match loaded_result {
                    Ok(images) => {
                        for (ix, image) in images {
                            if let Some(page) = this.pages.get_mut(ix) {
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
                    this.thumbnail_loading.remove(&ix);
                    if !loaded_indices.contains(&ix)
                        && let Some(page) = this.pages.get_mut(ix)
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
        if visible_range.is_empty() || self.pages.is_empty() {
            return;
        }

        if self.thumbnail_inflight_tasks == 0 && !self.thumbnail_loading.is_empty() {
            self.thumbnail_loading.clear();
        }

        self.last_thumbnail_visible_range = Some(visible_range.clone());
        self.trim_thumbnail_cache(visible_range.clone());

        let mut candidate_order = Vec::with_capacity(visible_range.len());
        candidate_order.extend(visible_range.clone());
        self.request_thumbnail_load_from_candidates(candidate_order, target_width, cx);
    }

    fn trim_thumbnail_cache(&mut self, visible_range: std::ops::Range<usize>) {
        let _ = visible_range;
    }

    fn display_available_width(&self, window: &Window) -> f32 {
        let viewport_width: f32 = window.viewport_size().width.into();
        (viewport_width - SIDEBAR_WIDTH).max(DISPLAY_MIN_WIDTH)
    }

    fn display_panel_width(&self, window: &Window) -> f32 {
        let available_width = self.display_available_width(window);
        (available_width * self.zoom).clamp(DISPLAY_MIN_WIDTH, available_width)
    }

    fn display_base_width(&self, window: &Window) -> f32 {
        self.display_panel_width(window)
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

    fn display_item_sizes(&self, base_width: f32) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            self.pages
                .iter()
                .map(|page| size(px(0.), px(self.display_row_height(page, base_width))))
                .collect(),
        )
    }

    fn display_target_width(&self, window: &Window) -> u32 {
        let width = self.display_panel_width(window) * window.scale_factor();
        width.clamp(1.0, i32::MAX as f32).round() as u32
    }

    fn request_display_load_from_candidates(
        &mut self,
        candidate_order: Vec<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        if candidate_order.is_empty() || self.pages.is_empty() {
            return;
        }

        let Some(path) = self.path.clone() else {
            return;
        };

        if self.display_inflight_tasks >= DISPLAY_MAX_PARALLEL_TASKS {
            return;
        }

        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for ix in candidate_order {
            if !seen.insert(ix) {
                continue;
            }

            let Some(page) = self.pages.get(ix) else {
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
            self.display_loading.insert(*ix);
        }
        self.display_inflight_tasks = self.display_inflight_tasks.saturating_add(1);
        let language = self.language;

        let epoch = self.display_epoch;
        cx.spawn(async move |view, cx| {
            let load_result = cx
                .background_executor()
                .spawn(async move {
                    let loaded = load_display_images(&path, &pending, target_width, language);
                    (pending, target_width, loaded)
                })
                .await;

            let _ = view.update(cx, |this, cx| {
                this.display_inflight_tasks = this.display_inflight_tasks.saturating_sub(1);
                if this.display_epoch != epoch {
                    return;
                }

                let (requested_indices, loaded_target_width, loaded_result) = load_result;
                let mut loaded_indices = HashSet::new();

                match loaded_result {
                    Ok(images) => {
                        for (ix, image) in images {
                            if let Some(page) = this.pages.get_mut(ix) {
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
                    this.display_loading.remove(&ix);
                    if !loaded_indices.contains(&ix)
                        && let Some(page) = this.pages.get_mut(ix)
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
        if visible_range.is_empty() || self.pages.is_empty() {
            return;
        }

        if self.display_inflight_tasks == 0 && !self.display_loading.is_empty() {
            self.display_loading.clear();
        }

        self.last_display_visible_range = Some(visible_range.clone());

        self.trim_display_cache(visible_range.clone());

        let mut candidate_order = Vec::with_capacity(visible_range.len());
        candidate_order.extend(visible_range.clone());

        self.request_display_load_from_candidates(candidate_order, target_width, cx);
    }

    fn trim_display_cache(&mut self, visible_range: std::ops::Range<usize>) {
        let _ = visible_range;
    }
}

impl Render for PdfViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(cx.theme().font_size);

        let page_count = self.pages.len();
        let current_page_num = if page_count == 0 {
            0
        } else {
            self.active_page + 1
        };
        let recent_popup_open = self.recent_popup_open();
        let recent_files = self.recent_files.clone();
        let file_name = self
            .path
            .as_ref()
            .map(|p| display_file_name(p))
            .unwrap_or_else(|| self.i18n().file_not_opened().to_string());
        let zoom_label: SharedString = format!("{:.0}%", self.zoom * 100.0).into();

        self.last_display_target_width = self.display_target_width(window);
        self.on_display_scroll_offset_changed(cx);

        let display_base_width = self.display_base_width(window);
        let display_panel_width = self.display_panel_width(window);
        let thumbnail_sizes = self.thumbnail_item_sizes();
        let display_sizes = self.display_item_sizes(display_base_width);

        window_border().child(
            div()
                .v_flex()
                .size_full()
                .bg(cx.theme().background)
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
                                        .child("KPDF"),
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
                        )),
                ),
        )
    }
}
