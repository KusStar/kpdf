impl PdfViewer {
    const LOCAL_STATE_DB_DIR_NAME: &'static str = "kpdf_db";

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
        Option<sled::Tree>,
        Option<sled::Tree>,
        Option<sled::Tree>,
    ) {
        let db_path = Self::local_state_db_path();
        if let Some(parent) = db_path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                crate::debug_log!("[store] create dir failed: {}", parent.to_string_lossy());
                return (None, None, None, None, None, None, None, None, None);
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
                return (None, None, None, None, None, None, None, None, None);
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
        let theme_preferences_store = match db.open_tree(THEME_PREFERENCES_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!(
                    "[store] open tree failed: {} | {}",
                    THEME_PREFERENCES_TREE,
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
        let notes_store = match db.open_tree(NOTES_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!("[store] open tree failed: {} | {}", NOTES_TREE, err);
                None
            }
        };
        let text_markups_store = match db.open_tree(TEXT_MARKUPS_TREE) {
            Ok(tree) => Some(tree),
            Err(err) => {
                crate::debug_log!("[store] open tree failed: {} | {}", TEXT_MARKUPS_TREE, err);
                None
            }
        };

        crate::debug_log!(
            "[store] init recent={} positions={} window_size={} open_tabs={} titlebar_preferences={} theme_preferences={} bookmarks={} notes={} text_markups={} path={}",
            recent_store.is_some(),
            position_store.is_some(),
            window_size_store.is_some(),
            open_tabs_store.is_some(),
            titlebar_preferences_store.is_some(),
            theme_preferences_store.is_some(),
            bookmarks_store.is_some(),
            notes_store.is_some(),
            text_markups_store.is_some(),
            db_path.to_string_lossy()
        );

        (
            recent_store,
            position_store,
            window_size_store,
            open_tabs_store,
            titlebar_preferences_store,
            theme_preferences_store,
            bookmarks_store,
            notes_store,
            text_markups_store,
        )
    }

    fn local_state_db_path() -> PathBuf {
        if let Some(app_data) = std::env::var_os("APPDATA") {
            return PathBuf::from(app_data)
                .join("kpdf")
                .join(Self::LOCAL_STATE_DB_DIR_NAME);
        }

        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".kpdf")
                .join(Self::LOCAL_STATE_DB_DIR_NAME);
        }

        PathBuf::from(".kpdf").join(Self::LOCAL_STATE_DB_DIR_NAME)
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

    fn load_markdown_notes_from_store(store: &sled::Tree) -> Vec<MarkdownNoteEntry> {
        let mut indexed_notes = Vec::new();
        for entry in store.iter() {
            let (key, value) = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if key.len() != 4 {
                continue;
            }
            let note_index = u32::from_be_bytes([key[0], key[1], key[2], key[3]]) as usize;
            let note = match serde_json::from_slice::<MarkdownNoteEntry>(&value) {
                Ok(note) => note,
                Err(_) => continue,
            };
            if note.markdown.trim().is_empty() {
                continue;
            }
            indexed_notes.push((note_index, note));
        }
        indexed_notes.sort_by_key(|(index, _)| *index);
        indexed_notes
            .into_iter()
            .map(|(_, note)| note)
            .collect::<Vec<_>>()
    }

    fn load_text_markups_from_store(store: &sled::Tree) -> Vec<TextMarkupEntry> {
        let mut indexed_markups = Vec::new();
        for entry in store.iter() {
            let (key, value) = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if key.len() != 4 {
                continue;
            }
            let markup_index = u32::from_be_bytes([key[0], key[1], key[2], key[3]]) as usize;
            let markup = match serde_json::from_slice::<TextMarkupEntry>(&value) {
                Ok(markup) => markup,
                Err(_) => continue,
            };
            if markup.rects.is_empty() {
                continue;
            }
            indexed_markups.push((markup_index, markup));
        }
        indexed_markups.sort_by_key(|(index, _)| *index);
        indexed_markups
            .into_iter()
            .map(|(_, markup)| markup)
            .collect::<Vec<_>>()
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

    fn decode_stored_string(value: Option<sled::IVec>) -> Option<String> {
        let raw = value?;
        let value = String::from_utf8(raw.to_vec()).ok()?;
        if value.is_empty() {
            return None;
        }
        Some(value)
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

    fn load_theme_preferences_from_store(
        store: &sled::Tree,
        default_mode: ThemeMode,
    ) -> (ThemeMode, Option<String>, Option<String>) {
        let mode = match store.get(THEME_PREFERENCES_KEY_MODE).ok().flatten() {
            Some(raw) => match raw.as_ref() {
                b"light" => ThemeMode::Light,
                b"dark" => ThemeMode::Dark,
                _ => default_mode,
            },
            None => default_mode,
        };
        let light_name =
            Self::decode_stored_string(store.get(THEME_PREFERENCES_KEY_LIGHT_NAME).ok().flatten());
        let dark_name =
            Self::decode_stored_string(store.get(THEME_PREFERENCES_KEY_DARK_NAME).ok().flatten());
        (mode, light_name, dark_name)
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

    fn persist_theme_preferences(&self) {
        let Some(store) = self.theme_preferences_store.as_ref() else {
            return;
        };

        let stored_value = match self.theme_mode {
            ThemeMode::Light => b"light".as_slice(),
            ThemeMode::Dark => b"dark".as_slice(),
        };
        if store
            .insert(THEME_PREFERENCES_KEY_MODE, stored_value)
            .is_err()
        {
            return;
        }
        if let Some(light_name) = self.preferred_light_theme_name.as_ref() {
            if store
                .insert(THEME_PREFERENCES_KEY_LIGHT_NAME, light_name.as_bytes())
                .is_err()
            {
                return;
            }
        } else if store.remove(THEME_PREFERENCES_KEY_LIGHT_NAME).is_err() {
            return;
        }
        if let Some(dark_name) = self.preferred_dark_theme_name.as_ref() {
            if store
                .insert(THEME_PREFERENCES_KEY_DARK_NAME, dark_name.as_bytes())
                .is_err()
            {
                return;
            }
        } else if store.remove(THEME_PREFERENCES_KEY_DARK_NAME).is_err() {
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

    fn persist_markdown_notes(&self) {
        let Some(store) = self.notes_store.as_ref() else {
            return;
        };

        if store.clear().is_err() {
            return;
        }

        for (index, note) in self.markdown_notes.iter().enumerate() {
            let key = (index as u32).to_be_bytes();
            let Ok(value) = serde_json::to_vec(note) else {
                continue;
            };
            if store.insert(key, value).is_err() {
                return;
            }
        }

        let _ = store.flush();
    }

    fn persist_text_markups(&self) {
        let Some(store) = self.text_markups_store.as_ref() else {
            return;
        };

        if store.clear().is_err() {
            return;
        }

        for (index, markup) in self.text_markups.iter().enumerate() {
            let key = (index as u32).to_be_bytes();
            let Ok(value) = serde_json::to_vec(markup) else {
                continue;
            };
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
}
