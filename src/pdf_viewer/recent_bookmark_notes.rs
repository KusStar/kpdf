impl PdfViewer {
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

    pub(super) fn insert_bookmark_and_notify(&mut self, cx: &mut Context<Self>, entry: BookmarkEntry) {
        self.bookmarks
            .retain(|item| !(item.path == entry.path && item.page_index == entry.page_index));
        self.bookmarks.insert(0, entry);
        self.persist_bookmarks();
        cx.notify();
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
        self.bookmark_popup_expanded_notes = None;
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
        if self.bookmark_popup_expanded_notes.take().is_some() {
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
                self.bookmark_popup_expanded_notes = None;
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
                if this.bookmark_popup_open || this.bookmark_popup_expanded_notes.is_some() {
                    this.bookmark_popup_open = false;
                    this.bookmark_popup_expanded_notes = None;
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
        self.bookmark_popup_expanded_notes = None;
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
            if self
                .bookmark_popup_expanded_notes
                .as_ref()
                .is_some_and(|(path, page_index)| {
                    path == &bookmark.path && *page_index == bookmark.page_index
                })
            {
                self.bookmark_popup_expanded_notes = None;
            }
            self.persist_bookmarks();
            cx.notify();
        }
    }

    fn next_markdown_note_id(&self) -> u64 {
        let mut candidate = Self::now_unix_millis().saturating_mul(1000);
        while self.markdown_notes.iter().any(|note| note.id == candidate) {
            candidate = candidate.saturating_add(1);
        }
        candidate
    }

    pub(super) fn active_tab_markdown_notes_for_page(
        &self,
        page_index: usize,
    ) -> Vec<MarkdownNoteEntry> {
        let Some(path) = self.active_tab_path() else {
            return Vec::new();
        };
        self.markdown_notes
            .iter()
            .filter(|note| note.path == *path && note.page_index == page_index)
            .cloned()
            .collect::<Vec<_>>()
    }

    fn bookmark_notes_for_entry(
        bookmark: &BookmarkEntry,
        markdown_notes: &[MarkdownNoteEntry],
    ) -> Vec<MarkdownNoteEntry> {
        markdown_notes
            .iter()
            .filter(|note| note.path == bookmark.path && note.page_index == bookmark.page_index)
            .cloned()
            .collect::<Vec<_>>()
    }

    fn bookmark_note_preview_text(markdown: &str) -> String {
        markdown
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default()
            .to_string()
    }

    fn toggle_bookmark_notes_list(&mut self, bookmark: &BookmarkEntry, cx: &mut Context<Self>) {
        let key = (bookmark.path.clone(), bookmark.page_index);
        if self.bookmark_popup_expanded_notes.as_ref() == Some(&key) {
            self.bookmark_popup_expanded_notes = None;
        } else {
            self.bookmark_popup_expanded_notes = Some(key);
        }
        cx.notify();
    }

    fn markdown_note_by_id(&self, note_id: u64) -> Option<MarkdownNoteEntry> {
        self.markdown_notes
            .iter()
            .find(|note| note.id == note_id)
            .cloned()
    }

    pub(super) fn hovered_markdown_note_id(&self) -> Option<u64> {
        self.hovered_markdown_note_id
    }

    pub(super) fn set_markdown_note_hover_id(&mut self, note_id: Option<u64>) -> bool {
        if self.hovered_markdown_note_id != note_id {
            self.hovered_markdown_note_id = note_id;
            true
        } else {
            false
        }
    }

    fn upsert_markdown_note(&mut self, note: MarkdownNoteEntry, cx: &mut Context<Self>) {
        self.markdown_notes.retain(|item| item.id != note.id);
        self.markdown_notes.insert(0, note);
        self.markdown_notes.sort_by(|a, b| {
            b.updated_at_unix_secs
                .cmp(&a.updated_at_unix_secs)
                .then_with(|| b.id.cmp(&a.id))
        });
        self.persist_markdown_notes();
        cx.notify();
    }

    fn delete_markdown_note_by_id(&mut self, note_id: u64, cx: &mut Context<Self>) {
        let original_len = self.markdown_notes.len();
        self.markdown_notes.retain(|note| note.id != note_id);
        if self.markdown_notes.len() != original_len {
            let _ = self.set_markdown_note_hover_id(None);
            self.persist_markdown_notes();
            cx.notify();
        }
    }

    pub(super) fn open_page_context_menu(
        &mut self,
        position: Point<Pixels>,
        note_anchor: Option<MarkdownNoteAnchor>,
        note_id: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let _ = self.set_markdown_note_hover_id(note_id);
        self.clear_text_selection_hover_menu_state();
        self.context_menu_open = true;
        self.context_menu_position = Some(position);
        self.context_menu_tab_id = None;
        self.context_menu_note_anchor = note_anchor;
        self.context_menu_note_id = note_id;
        cx.notify();
    }

    pub(super) fn render_bookmark_popup_panel(
        popup_id: &'static str,
        i18n: I18n,
        viewer: Entity<Self>,
        scope: BookmarkScope,
        bookmarks: Vec<BookmarkEntry>,
        bookmark_notes: Vec<MarkdownNoteEntry>,
        expanded_notes: Option<(PathBuf, usize)>,
        scroll_handle: &ScrollHandle,
        cx: &mut Context<PopoverState>,
    ) -> AnyElement {
        let now_unix_secs = Self::now_unix_secs();

        // 如果没有书签，获取有笔记的页面并转换成书签样式
        let bookmarks_to_show = if bookmarks.is_empty() {
            viewer
                .read(cx)
                .bookmarks_from_notes(scope, &bookmark_notes)
        } else {
            bookmarks.clone()
        };

        // 如果没有书签也没有笔记，显示空状态
        let show_empty = bookmarks_to_show.is_empty();

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
                                .label(i18n.bookmark_scope_current_pdf)
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
                                .label(i18n.bookmark_scope_all)
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
            .when(show_empty, |this| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(i18n.no_bookmarks),
                )
            })
            .when(!show_empty, |this| {
                this.child(Self::render_bookmark_list_content(
                    i18n,
                    viewer.clone(),
                    &bookmarks_to_show,
                    &bookmark_notes,
                    expanded_notes,
                    scroll_handle,
                    now_unix_secs,
                    cx,
                ))
            })
            .into_any_element()
    }

    /// 从笔记创建书签列表（当没有书签时）
    fn bookmarks_from_notes(
        &self,
        scope: BookmarkScope,
        markdown_notes: &[MarkdownNoteEntry],
    ) -> Vec<BookmarkEntry> {
        let current_path = self.active_tab_path().cloned();
        let mut pages: Vec<(PathBuf, usize)> = markdown_notes
            .iter()
            .map(|note| (note.path.clone(), note.page_index))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // 根据 scope 过滤
        if scope == BookmarkScope::CurrentPdf {
            if let Some(current_path) = current_path {
                pages.retain(|(path, _)| path == &current_path);
            }
        }

        // 按路径和页码排序
        pages.sort_by(|a, b| {
            a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1))
        });

        // 转换成 BookmarkEntry
        pages
            .into_iter()
            .map(|(path, page_index)| BookmarkEntry {
                path,
                page_index,
                created_at_unix_secs: 0,
            })
            .collect()
    }

    /// 渲染书签列表内容
    fn render_bookmark_list_content(
        i18n: I18n,
        viewer: Entity<Self>,
        bookmarks: &[BookmarkEntry],
        bookmark_notes: &[MarkdownNoteEntry],
        expanded_notes: Option<(PathBuf, usize)>,
        scroll_handle: &ScrollHandle,
        _now_unix_secs: u64,
        cx: &mut Context<PopoverState>,
    ) -> AnyElement {
        let is_from_notes = bookmarks.iter().any(|b| b.created_at_unix_secs == 0);

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
                    .overflow_x_hidden()
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
                                let bookmark_for_note_toggle = bookmark.clone();
                                let file_name = display_file_name(&bookmark.path);
                                let page_label = i18n.bookmark_page_label(bookmark.page_index + 1);
                                let notes_for_bookmark = Self::bookmark_notes_for_entry(&bookmark, bookmark_notes);
                                let notes_count = notes_for_bookmark.len();
                                let is_notes_expanded = expanded_notes
                                    .as_ref()
                                    .is_some_and(|(path, page_index)| {
                                        *path == bookmark.path && *page_index == bookmark.page_index
                                    });
                                let notes_count_label = i18n.bookmark_notes_count_label(notes_count);

                                div()
                                    .id(("bookmark-item", ix))
                                    .w_full()
                                    .rounded_md()
                                    .px_2()
                                    .py_1()
                                    .cursor_pointer()
                                    .hover(|this| this.bg(cx.theme().secondary.opacity(0.6)))
                                    .active(|this| this.bg(cx.theme().secondary.opacity(0.9)))
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .items_start()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_px()
                                                    .overflow_x_hidden()
                                                    .v_flex()
                                                    .items_start()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .flex()
                                                            .items_center()
                                                            .justify_between()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .text_color(cx.theme().popover_foreground)
                                                                    .child(page_label),
                                                            )
                                                            .child(
                                                                div()
                                                                    .min_w_px()
                                                                    .truncate()
                                                                    .text_xs()
                                                                    .text_color(
                                                                        if notes_count == 0 {
                                                                            cx.theme().muted_foreground
                                                                        } else if is_notes_expanded {
                                                                            cx.theme().primary
                                                                        } else {
                                                                            cx.theme().foreground.opacity(0.75)
                                                                        },
                                                                    )
                                                                    .when(notes_count > 0, |this| {
                                                                        this.cursor_pointer().on_mouse_down(MouseButton::Left, {
                                                                            let viewer = viewer.clone();
                                                                            let bookmark = bookmark_for_note_toggle.clone();
                                                                            cx.listener(move |_, _: &MouseDownEvent, _, cx| {
                                                                                cx.stop_propagation();
                                                                                let _ = viewer.update(cx, |this, cx| {
                                                                                    this.toggle_bookmark_notes_list(&bookmark, cx);
                                                                                });
                                                                            })
                                                                        })
                                                                    })
                                                                    .child(notes_count_label),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .truncate()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(file_name),
                                                    ),
                                            ),
                                    )
                                    .when(is_notes_expanded && !notes_for_bookmark.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .w_full()
                                                .v_flex()
                                                .gap_1()
                                                .children(
                                                    notes_for_bookmark
                                                        .iter()
                                                        .enumerate()
                                                        .map(|(note_ix, note)| {
                                                            let preview = Self::bookmark_note_preview_text(&note.markdown);
                                                            let label = if preview.is_empty() {
                                                                format!("{}. {}", note_ix + 1, note.markdown.trim())
                                                            } else {
                                                                format!("{}. {}", note_ix + 1, preview)
                                                            };
                                                            div()
                                                                .w_full()
                                                                .truncate()
                                                                .text_xs()
                                                                .text_color(cx.theme().foreground.opacity(0.75))
                                                                .child(label)
                                                        })
                                                        .collect::<Vec<_>>(),
                                                ),
                                        )
                                    })
                                    .when(is_from_notes, |this| {
                                        // 如果是来自笔记的书签，不显示删除按钮
                                        this
                                    })
                                    .when(!is_from_notes, |this| {
                                        this.child(
                                            Button::new(("bookmark-delete", ix))
                                                .xsmall()
                                                .ghost()
                                                .icon(Icon::new(crate::icons::IconName::BookmarkMinus).size_4().text_color(cx.theme().muted_foreground))
                                                .on_click({
                                                    let viewer = viewer.clone();
                                                    move |_, _, cx| {
                                                        let bookmark = bookmark_for_delete.clone();
                                                        let _ = viewer.update(cx, |this, cx| {
                                                            this.delete_bookmark(&bookmark, cx);
                                                        });
                                                    }
                                                }),
                                        )
                                    })
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
            )
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
                        .label(i18n.choose_file_button)
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
                        .child(i18n.no_recent_files),
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
}
