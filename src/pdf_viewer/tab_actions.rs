impl PdfViewer {
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

        let _ = self.set_markdown_note_hover_id(None);

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
            let _ = self.set_markdown_note_hover_id(None);
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

        if self.note_editor_open {
            if key == "escape" {
                self.close_markdown_note_editor(cx);
                cx.stop_propagation();
                return;
            }
            if key == "enter" && is_primary_modifier {
                self.save_markdown_note_from_editor(cx);
                cx.stop_propagation();
                return;
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

}
