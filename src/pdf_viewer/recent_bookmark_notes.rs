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
                if this.bookmark_popup_open {
                    this.bookmark_popup_open = false;
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

    fn upsert_markdown_note(&mut self, note: MarkdownNoteEntry) {
        self.markdown_notes.retain(|item| item.id != note.id);
        self.markdown_notes.insert(0, note);
        self.markdown_notes.sort_by(|a, b| {
            b.updated_at_unix_secs
                .cmp(&a.updated_at_unix_secs)
                .then_with(|| b.id.cmp(&a.id))
        });
        self.persist_markdown_notes();
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

    fn open_markdown_note_editor_for_new(
        &mut self,
        anchor: MarkdownNoteAnchor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.note_editor_anchor = Some(anchor);
        self.note_editor_edit_note_id = None;
        self.note_editor_open = true;
        self.note_editor_needs_focus = true;
        self.needs_root_refocus = false;
        self.note_editor_input_state.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        cx.notify();
    }

    fn open_markdown_note_editor_for_edit(
        &mut self,
        note_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(note) = self.markdown_note_by_id(note_id) else {
            return;
        };
        self.note_editor_anchor = None;
        self.note_editor_edit_note_id = Some(note_id);
        self.note_editor_open = true;
        self.note_editor_needs_focus = true;
        self.needs_root_refocus = false;
        self.note_editor_input_state.update(cx, |input, cx| {
            input.set_value(note.markdown, window, cx);
        });
        cx.notify();
    }

    fn close_markdown_note_editor(&mut self, cx: &mut Context<Self>) {
        if !self.note_editor_open {
            return;
        }
        self.note_editor_open = false;
        self.note_editor_anchor = None;
        self.note_editor_edit_note_id = None;
        self.note_editor_needs_focus = false;
        self.needs_root_refocus = true;
        cx.notify();
    }

    fn save_markdown_note_from_editor(&mut self, cx: &mut Context<Self>) {
        if !self.note_editor_open {
            return;
        }
        let markdown = self.note_editor_input_state.read(cx).value().to_string();
        let markdown = markdown.trim().to_string();
        if markdown.is_empty() {
            return;
        }

        let now = Self::now_unix_secs();
        if let Some(note_id) = self.note_editor_edit_note_id
            && let Some(mut note) = self.markdown_note_by_id(note_id)
        {
            note.markdown = markdown;
            note.updated_at_unix_secs = now;
            self.upsert_markdown_note(note);
            self.close_markdown_note_editor(cx);
            cx.notify();
            return;
        }

        let Some(path) = self.active_tab_path().cloned() else {
            return;
        };
        let Some(anchor) = self.note_editor_anchor else {
            return;
        };

        let note = MarkdownNoteEntry {
            id: self.next_markdown_note_id(),
            path,
            page_index: anchor.page_index,
            x_ratio: anchor.x_ratio.clamp(0.0, 1.0),
            y_ratio: anchor.y_ratio.clamp(0.0, 1.0),
            markdown,
            created_at_unix_secs: now,
            updated_at_unix_secs: now,
        };
        self.upsert_markdown_note(note);
        self.close_markdown_note_editor(cx);
        cx.notify();
    }

    pub(super) fn open_page_context_menu(
        &mut self,
        position: Point<Pixels>,
        note_anchor: Option<MarkdownNoteAnchor>,
        note_id: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let _ = self.set_markdown_note_hover_id(note_id);
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
        scroll_handle: &ScrollHandle,
        cx: &mut Context<PopoverState>,
    ) -> AnyElement {
        let now_unix_secs = Self::now_unix_secs();
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
            .when(bookmarks.is_empty(), |this| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(i18n.no_bookmarks),
                )
            })
            .when(!bookmarks.is_empty(), |this| {
                this.child(
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
                                            let file_name = display_file_name(&bookmark.path);
                                            let page_label =
                                                i18n.bookmark_page_label(bookmark.page_index + 1);
                                            let path_text = bookmark.path.display().to_string();
                                            let added_time_label =
                                                if bookmark.created_at_unix_secs == 0 {
                                                    i18n.bookmark_added_unknown.to_string()
                                                } else {
                                                    i18n.bookmark_added_relative(
                                                        now_unix_secs.saturating_sub(
                                                            bookmark.created_at_unix_secs,
                                                        ),
                                                    )
                                                };
                                            div()
                                                .id(("bookmark-item", ix))
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
                                                        .flex()
                                                        .items_start()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .v_flex()
                                                                .items_start()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .text_sm()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .popover_foreground,
                                                                        )
                                                                        .child(page_label),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .whitespace_normal()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .child(file_name),
                                                                )
                                                                .when(
                                                                    scope == BookmarkScope::All,
                                                                    |this| {
                                                                        this.child(
                                                                            div()
                                                                                .w_full()
                                                                                .whitespace_normal()
                                                                                .text_xs()
                                                                                .text_color(
                                                                                    cx.theme()
                                                                                        .muted_foreground,
                                                                                )
                                                                                .child(path_text),
                                                                        )
                                                                    },
                                                                )
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .whitespace_normal()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .child(added_time_label),
                                                                ),
                                                        )
                                                        .child(
                                                            Button::new(("bookmark-delete", ix))
                                                                .xsmall()
                                                                .ghost()
                                                                .icon(
                                                                    Icon::new(
                                                                        crate::icons::IconName::BookmarkMinus,
                                                                    )
                                                                    .size_4()
                                                                    .text_color(
                                                                        cx.theme().muted_foreground,
                                                                    ),
                                                                )
                                                                .on_click({
                                                                    let viewer = viewer.clone();
                                                                    move |_, _, cx| {
                                                                        let bookmark =
                                                                            bookmark_for_delete
                                                                                .clone();
                                                                        let _ = viewer.update(
                                                                            cx,
                                                                            |this, cx| {
                                                                                this.delete_bookmark(
                                                                                    &bookmark, cx,
                                                                                );
                                                                            },
                                                                        );
                                                                    }
                                                                }),
                                                        ),
                                                )
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
                        ),
                )
            })
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
