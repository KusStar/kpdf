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

    pub fn check_updates_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "检查更新...",
            Language::EnUs => "Check for Updates...",
        }
    }

    pub fn settings_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "设置...",
            Language::EnUs => "Settings...",
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

    pub fn updates_label(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "更新",
            Language::EnUs => "Updates",
        }
    }

    pub fn update_status_idle(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "尚未检查",
            Language::EnUs => "Not checked yet",
        }
    }

    pub fn update_status_checking(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "正在检查更新...",
            Language::EnUs => "Checking for updates...",
        }
    }

    pub fn update_status_up_to_date(self, version: &str) -> String {
        match self.lang {
            Language::ZhCn => format!("当前已是最新版本（{}）", version),
            Language::EnUs => format!("You're up to date ({version})"),
        }
    }

    pub fn update_status_available(self, version: &str) -> String {
        match self.lang {
            Language::ZhCn => format!("发现新版本：{}", version),
            Language::EnUs => format!("Update available: {version}"),
        }
    }

    pub fn update_status_failed(self, message: &str) -> String {
        match self.lang {
            Language::ZhCn => format!("检查失败：{}", message),
            Language::EnUs => format!("Check failed: {message}"),
        }
    }

    pub fn download_update_button(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "下载更新",
            Language::EnUs => "Download Update",
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

    pub fn settings_dialog_title(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "设置",
            Language::EnUs => "Settings",
        }
    }

    pub fn settings_titlebar_section(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "标题栏",
            Language::EnUs => "Title Bar",
        }
    }

    pub fn settings_titlebar_navigation_label(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "页面导航",
            Language::EnUs => "Page Navigation",
        }
    }

    pub fn settings_titlebar_navigation_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "显示首页/上一页/页码/下一页/末页控件",
            Language::EnUs => "Show first/prev/page/next/last controls",
        }
    }

    pub fn settings_titlebar_zoom_label(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "缩放控件",
            Language::EnUs => "Zoom Controls",
        }
    }

    pub fn settings_titlebar_zoom_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "显示缩小/默认/放大按钮与缩放百分比",
            Language::EnUs => "Show zoom out/reset/in buttons and zoom percentage",
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

    pub fn command_panel_menu_badge(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "菜单",
            Language::EnUs => "Menu",
        }
    }

    pub fn command_panel_open_about_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "打开关于窗口",
            Language::EnUs => "Open About dialog",
        }
    }

    pub fn command_panel_check_updates_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "检查最新版本",
            Language::EnUs => "Check for updates",
        }
    }

    pub fn command_panel_open_settings_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "打开设置窗口",
            Language::EnUs => "Open Settings dialog",
        }
    }

    pub fn command_panel_open_logs_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "打开日志目录",
            Language::EnUs => "Open logs folder",
        }
    }

    pub fn command_panel_enable_logging_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "启用文件日志记录",
            Language::EnUs => "Enable file logging",
        }
    }

    pub fn command_panel_disable_logging_hint(self) -> &'static str {
        match self.lang {
            Language::ZhCn => "关闭文件日志记录",
            Language::EnUs => "Disable file logging",
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
