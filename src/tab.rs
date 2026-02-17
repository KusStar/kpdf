use crate::pdf_viewer::text_selection::TextSelectionManager;
use crate::pdf_viewer::PageSummary;
use gpui::*;
use gpui_component::VirtualListScrollHandle;
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone)]
pub struct PdfTab {
    pub id: usize,
    pub path: Option<PathBuf>,
    pub pages: Vec<PageSummary>,
    pub selected_page: usize,
    pub active_page: usize,
    pub zoom: f32,
    pub thumbnail_scroll: VirtualListScrollHandle,
    pub display_scroll: VirtualListScrollHandle,
    pub thumbnail_loading: HashSet<usize>,
    pub thumbnail_inflight_tasks: usize,
    pub thumbnail_epoch: u64,
    pub last_thumbnail_visible_range: Option<std::ops::Range<usize>>,
    pub display_loading: HashSet<usize>,
    pub display_inflight_tasks: usize,
    pub display_epoch: u64,
    pub last_display_visible_range: Option<std::ops::Range<usize>>,
    pub last_display_target_width: u32,
    pub display_scroll_sync_epoch: u64,
    pub last_display_scroll_offset: Option<Point<Pixels>>,
    pub suppress_display_scroll_sync_once: bool,
    pub last_saved_position: Option<(PathBuf, usize)>,
    pub text_selection_manager: RefCell<TextSelectionManager>,
}

impl PdfTab {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            path: None,
            pages: Vec::new(),
            selected_page: 0,
            active_page: 0,
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
            last_display_target_width: 220,
            display_scroll_sync_epoch: 0,
            last_display_scroll_offset: None,
            suppress_display_scroll_sync_once: false,
            last_saved_position: None,
            text_selection_manager: RefCell::new(TextSelectionManager::new()),
        }
    }

    pub fn reset_thumbnail_render_state(&mut self) {
        self.thumbnail_loading.clear();
        self.thumbnail_inflight_tasks = 0;
        self.thumbnail_epoch = self.thumbnail_epoch.wrapping_add(1);
        self.last_thumbnail_visible_range = None;
    }

    pub fn reset_display_render_state(&mut self) {
        self.display_loading.clear();
        self.display_inflight_tasks = 0;
        self.display_epoch = self.display_epoch.wrapping_add(1);
        self.last_display_visible_range = None;
    }

    pub fn reset_page_render_state(&mut self) {
        self.reset_thumbnail_render_state();
        self.reset_display_render_state();
        self.text_selection_manager.borrow_mut().clear_cache();
        self.text_selection_manager.borrow_mut().clear_selection();
    }

    pub fn file_name(&self) -> String {
        self.path
            .as_ref()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string())
            })
            .unwrap_or_else(|| "Home".to_string())
    }
}

pub struct TabBar {
    tabs: Vec<PdfTab>,
    active_tab_id: Option<usize>,
    next_tab_id: usize,
    // 注意：这些字段在当前实现中没有使用，因为我们完全在 PdfViewer 中处理拖动状态
    // 保留它们是为了将来可能的重构需求
    drag_source_index: Option<usize>,
    drag_target_index: Option<usize>,
    is_dragging: bool,
}

impl TabBar {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_id: None,
            next_tab_id: 1,
            drag_source_index: None,
            drag_target_index: None,
            is_dragging: false,
        }
    }

    pub fn create_tab(&mut self) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = PdfTab::new(id);
        self.tabs.push(tab);
        self.active_tab_id = Some(id);
        id
    }

    pub fn create_tab_with_path(&mut self, path: PathBuf, pages: Vec<PageSummary>) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let mut tab = PdfTab::new(id);
        tab.path = Some(path);
        tab.pages = pages;
        self.tabs.push(tab);
        self.active_tab_id = Some(id);
        id
    }

    pub fn close_tab(&mut self, tab_id: usize) -> bool {
        let index = self.tabs.iter().position(|t| t.id == tab_id);
        if let Some(index) = index {
            self.tabs.remove(index);

            // 更新活动标签页
            if self.active_tab_id == Some(tab_id) {
                if self.tabs.is_empty() {
                    self.active_tab_id = None;
                } else if index < self.tabs.len() {
                    self.active_tab_id = Some(self.tabs[index].id);
                } else {
                    self.active_tab_id = Some(self.tabs[self.tabs.len() - 1].id);
                }
            }
            true
        } else {
            false
        }
    }

    pub fn get_active_tab(&self) -> Option<&PdfTab> {
        self.active_tab_id
            .and_then(|id| self.tabs.iter().find(|t| t.id == id))
    }

    pub fn get_active_tab_mut(&mut self) -> Option<&mut PdfTab> {
        let active_id = self.active_tab_id?;
        self.tabs.iter_mut().find(|t| t.id == active_id)
    }

    pub fn switch_to_tab(&mut self, tab_id: usize) -> bool {
        if self.tabs.iter().any(|t| t.id == tab_id) {
            self.active_tab_id = Some(tab_id);
            true
        } else {
            false
        }
    }

    pub fn tabs(&self) -> &[PdfTab] {
        &self.tabs
    }

    pub fn active_tab_id(&self) -> Option<usize> {
        self.active_tab_id
    }

    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    #[allow(dead_code)]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Move a tab from one index to another
    pub fn move_tab(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index >= self.tabs.len() || to_index >= self.tabs.len() {
            return false;
        }

        if from_index == to_index {
            return true; // No movement needed
        }

        let tab = self.tabs.remove(from_index);
        self.tabs.insert(to_index, tab);
        true
    }

    /// Get mutable reference to tabs
    pub fn tabs_mut(&mut self) -> &mut Vec<PdfTab> {
        &mut self.tabs
    }
}

// 拖动相关方法
impl TabBar {
    pub fn start_drag(&mut self, tab_index: usize) {
        self.drag_source_index = Some(tab_index);
        self.is_dragging = true;
    }

    pub fn update_drag(&mut self, tab_index: usize) {
        if self.is_dragging {
            self.drag_target_index = Some(tab_index);
        }
    }

    pub fn end_drag(&mut self) -> Option<(usize, usize)> {
        let result = if let (Some(source_idx), Some(target_idx)) = (self.drag_source_index, self.drag_target_index) {
            if source_idx != target_idx {
                Some((source_idx, target_idx))
            } else {
                None
            }
        } else {
            None
        };

        self.drag_source_index = None;
        self.drag_target_index = None;
        self.is_dragging = false;

        if let Some((source_idx, target_idx)) = result {
            // 重新排列标签页
            if source_idx < target_idx {
                // 向右拖动：将源元素插入到目标位置之后
                let tab = self.tabs.remove(source_idx);
                self.tabs.insert(target_idx, tab);
            } else {
                // 向左拖动：将源元素插入到目标位置
                let tab = self.tabs.remove(source_idx);
                self.tabs.insert(target_idx, tab);
            }
            Some((source_idx, target_idx))
        } else {
            None
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }

    pub fn drag_target_index(&self) -> Option<usize> {
        self.drag_target_index
    }

    pub fn get_tab_index_by_id(&self, tab_id: usize) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == tab_id)
    }
}
