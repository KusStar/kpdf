use serde::{Deserialize, Serialize};

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
    pub(super) path: PathBuf,
    pub(super) page_index: usize,
    pub(super) created_at_unix_secs: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum BookmarkScope {
    CurrentPdf,
    All,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum LanguagePreference {
    System,
    ZhCn,
    EnUs,
}

impl Default for LanguagePreference {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct MarkdownNoteEntry {
    pub(super) id: u64,
    pub(super) path: PathBuf,
    pub(super) page_index: usize,
    pub(super) x_ratio: f32,
    pub(super) y_ratio: f32,
    pub(super) markdown: String,
    pub(super) created_at_unix_secs: u64,
    pub(super) updated_at_unix_secs: u64,
    /// 如果是从文本选区创建的，存储选中的文本和矩形区域
    #[serde(default)]
    pub(super) selected_text: String,
    #[serde(default)]
    pub(super) selection_rects: Vec<TextMarkupRect>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TextMarkupKind {
    Highlight,
    Underline,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(super) enum TextMarkupColor {
    #[default]
    Yellow,
    Green,
    Blue,
    Pink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct TextMarkupRect {
    pub(super) left_ratio: f32,
    pub(super) top_ratio: f32,
    pub(super) right_ratio: f32,
    pub(super) bottom_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct TextMarkupEntry {
    pub(super) id: u64,
    pub(super) path: PathBuf,
    pub(super) page_index: usize,
    pub(super) kind: TextMarkupKind,
    #[serde(default)]
    pub(super) color: TextMarkupColor,
    pub(super) selected_text: String,
    pub(super) rects: Vec<TextMarkupRect>,
    pub(super) created_at_unix_secs: u64,
    pub(super) updated_at_unix_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MarkdownNoteAnchor {
    pub(super) page_index: usize,
    pub(super) x_ratio: f32,
    pub(super) y_ratio: f32,
}

#[derive(Debug, Clone)]
pub(super) enum UpdaterUiState {
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
pub(super) struct TitleBarVisibilityPreferences {
    pub(super) show_navigation: bool,
    pub(super) show_zoom: bool,
}

impl Default for TitleBarVisibilityPreferences {
    fn default() -> Self {
        Self {
            show_navigation: true,
            show_zoom: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TabLayoutMode {
    Horizontal,
    Vertical,
}

impl Default for TabLayoutMode {
    fn default() -> Self {
        Self::Horizontal
    }
}
