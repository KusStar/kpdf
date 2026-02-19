impl PdfViewer {
    fn open_pdf_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.close_command_panel(cx);
        self.close_recent_popup(cx);
        self.close_bookmark_popup(cx);
        self.close_markdown_note_editor(cx);

        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(self.i18n().open_pdf_prompt.into()),
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
}
