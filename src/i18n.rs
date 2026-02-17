use std::path::Path;

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
}

#[derive(Clone, Copy, Debug)]
pub struct I18n {
    lang: Language,
}

impl I18n {
    pub fn new(lang: Language) -> Self {
        Self { lang }
    }

    pub fn file_not_opened(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "未打开文件",
            Language::EnUs => "No file opened",
        }
    }

    pub fn open_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "打开",
            Language::EnUs => "Open",
        }
    }

    pub fn choose_file_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "选择文件...",
            Language::EnUs => "Choose file...",
        }
    }

    pub fn no_recent_files(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "暂无最近文件",
            Language::EnUs => "No recent files",
        }
    }

    pub fn last_seen_page(self, page_num: usize) -> String {
        match self.lang {
            Language::ZhCn => format!("上次看到：第 {} 页", page_num),
            Language::EnUs => format!("Last seen: page {}", page_num),
        }
    }

    pub fn zoom_reset_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "默认",
            Language::EnUs => "Reset",
        }
    }

    pub fn open_logs_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "打开日志目录",
            Language::EnUs => "Open Logs",
        }
    }

    pub fn enable_logging_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "启用日志记录",
            Language::EnUs => "Enable Logging",
        }
    }

    pub fn disable_logging_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "关闭日志记录",
            Language::EnUs => "Disable Logging",
        }
    }

    pub fn about_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "关于 kPDF",
            Language::EnUs => "About kPDF",
        }
    }

    pub fn about_dialog_title(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "关于",
            Language::EnUs => "About",
        }
    }

    pub fn about_app_info(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "轻量级 PDF 阅读器",
            Language::EnUs => "A lightweight PDF viewer",
        }
    }

    pub fn version_label(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "版本",
            Language::EnUs => "Version",
        }
    }

    pub fn website_label(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "官网",
            Language::EnUs => "Website",
        }
    }

    pub fn open_website_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "打开官网",
            Language::EnUs => "Open Website",
        }
    }

    pub fn close_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "关闭",
            Language::EnUs => "Close",
        }
    }

    pub fn no_pages(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "暂无页面",
            Language::EnUs => "No pages",
        }
    }

    pub fn no_document_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "点击上方“打开”选择 PDF",
            Language::EnUs => "Click Open above to select a PDF",
        }
    }

    pub fn page_render_failed(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "页面渲染失败",
            Language::EnUs => "Failed to render page",
        }
    }

    pub fn thumbnail_render_failed(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "缩略图失败",
            Language::EnUs => "Failed to render thumbnail",
        }
    }

    pub fn page_badge(self, page_num: usize) -> String {
        match self.lang {
            Language::ZhCn => format!("第 {} 页", page_num),
            Language::EnUs => format!("Page {}", page_num),
        }
    }

    pub fn open_pdf_prompt(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "打开 PDF",
            Language::EnUs => "Open PDF",
        }
    }

    pub fn command_panel_title(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "Command Panel",
            Language::EnUs => "Command Panel",
        }
    }

    pub fn command_panel_search_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "输入文件名或路径快速过滤",
            Language::EnUs => "Type filename or path to filter",
        }
    }

    pub fn command_panel_open_files(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "已打开文件",
            Language::EnUs => "Open Files",
        }
    }

    pub fn command_panel_recent_files(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "最近打开",
            Language::EnUs => "Recent Files",
        }
    }

    pub fn command_panel_no_open_files(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "暂无已打开文件",
            Language::EnUs => "No open files",
        }
    }

    pub fn command_panel_current_badge(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "当前",
            Language::EnUs => "Current",
        }
    }

    pub fn pdfium_not_found(self) -> &'static str {
        match self.lang {
            Language::ZhCn => {
                "未找到 Pdfium 动态库（已尝试 App 资源目录、可执行文件附近的 lib、当前目录与系统库）"
            }
            Language::EnUs => {
                "Pdfium dynamic library not found (tried app resources, lib near executable, working directory, and system library)"
            }
        }
    }

    pub fn cannot_open_file(self, path: &Path) -> String {
        match self.lang {
            Language::ZhCn => format!("无法打开文件: {}", path.to_string_lossy()),
            Language::EnUs => format!("Cannot open file: {}", path.to_string_lossy()),
        }
    }

    pub fn pdfium_cache_lock_poisoned(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "Pdfium 文档缓存锁已中毒",
            Language::EnUs => "Pdfium document cache lock is poisoned",
        }
    }

    pub fn pdfium_cannot_open_file(self, path: &Path) -> String {
        match self.lang {
            Language::ZhCn => format!("Pdfium 无法打开文件: {}", path.to_string_lossy()),
            Language::EnUs => format!("Pdfium cannot open file: {}", path.to_string_lossy()),
        }
    }

    pub fn invalid_bitmap_size(self, width: u32, height: u32) -> String {
        match self.lang {
            Language::ZhCn => format!("位图尺寸无效: {}x{}", width, height),
            Language::EnUs => format!("Invalid bitmap size: {}x{}", width, height),
        }
    }

    pub fn bitmap_len_mismatch(self, got: usize, expected: usize) -> String {
        match self.lang {
            Language::ZhCn => {
                format!("位图数据长度异常: got={}, expected={}", got, expected)
            }
            Language::EnUs => {
                format!(
                    "Bitmap byte length mismatch: got={}, expected={}",
                    got, expected
                )
            }
        }
    }

    pub fn copy_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "复制",
            Language::EnUs => "Copy",
        }
    }

    pub fn close_all_tabs_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "关闭所有标签页",
            Language::EnUs => "Close All Tabs",
        }
    }

    pub fn close_other_tabs_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "关闭其他标签页",
            Language::EnUs => "Close Other Tabs",
        }
    }

    pub fn reveal_in_file_manager_button(self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            return match self.lang {
                Language::ZhCn => "在 Finder 中显示",
                Language::EnUs => "Reveal in Finder",
            };
        }
        #[cfg(target_os = "windows")]
        {
            return match self.lang {
                Language::ZhCn => "在资源管理器中显示",
                Language::EnUs => "Reveal in Explorer",
            };
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            return match self.lang {
                Language::ZhCn => "打开所在文件夹",
                Language::EnUs => "Open Containing Folder",
            };
        }
        #[allow(unreachable_code)]
        match self.lang {
            Language::ZhCn => "打开所在文件夹",
            Language::EnUs => "Open Containing Folder",
        }
    }

    pub fn cannot_create_image_buffer(self, width: u32, height: u32) -> String {
        match self.lang {
            Language::ZhCn => format!("无法创建图像缓冲区: {}x{}", width, height),
            Language::EnUs => format!("Cannot create image buffer: {}x{}", width, height),
        }
    }
}
