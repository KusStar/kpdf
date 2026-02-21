impl PdfViewer {
    fn open_markdown_note_editor_for_new(
        &mut self,
        anchor: MarkdownNoteAnchor,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_markdown_note_editor(cx);
        self.note_editor_anchor = Some(anchor);
        self.note_editor_edit_note_id = None;
        self.open_markdown_note_editor_window(String::new(), false, cx);
    }

    pub(super) fn open_markdown_note_editor_for_text_selection(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let anchor = self
            .text_selection_hover_menu_anchor
            .or_else(|| self.active_text_selection_anchor());
        let Some(anchor) = anchor else {
            return false;
        };

        self.close_markdown_note_editor(cx);
        self.clear_text_selection_hover_menu_state();
        self.note_editor_anchor = Some(anchor);
        self.note_editor_edit_note_id = None;
        self.open_markdown_note_editor_window(String::new(), false, cx);
        cx.notify();
        true
    }

    fn open_markdown_note_editor_for_edit(
        &mut self,
        note_id: u64,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(note) = self.markdown_note_by_id(note_id) else {
            return;
        };
        self.close_markdown_note_editor(cx);
        self.note_editor_anchor = None;
        self.note_editor_edit_note_id = Some(note_id);
        self.open_markdown_note_editor_window(note.markdown, true, cx);
    }

    fn open_markdown_note_editor_window(
        &mut self,
        initial_markdown: String,
        is_editing: bool,
        cx: &mut Context<Self>,
    ) {
        self.note_editor_open = true;
        self.needs_root_refocus = false;
        self.note_editor_session = self.note_editor_session.wrapping_add(1);
        let session_id = self.note_editor_session;
        let language = self.language;
        let viewer = cx.entity();
        let viewer_for_close = viewer.clone();

        let window_options = WindowOptions {
            titlebar: Some(Self::dialog_titlebar_options()),
            window_bounds: Some(WindowBounds::centered(
                size(
                    px(MARKDOWN_NOTE_EDITOR_WIDTH + 64.),
                    px(MARKDOWN_NOTE_EDITOR_WINDOW_HEIGHT),
                ),
                cx,
            )),
            window_decorations: Some(WindowDecorations::Client),
            ..WindowOptions::default()
        };

        match cx.open_window(window_options, move |window, cx| {
            window.on_window_should_close(cx, move |_, cx| {
                let _ = viewer_for_close.update(cx, |this, cx| {
                    this.on_markdown_note_editor_window_closed(session_id, cx);
                });
                true
            });
            let editor = cx.new(|cx| {
                MarkdownNoteEditorWindow::new(
                    viewer,
                    session_id,
                    language,
                    is_editing,
                    initial_markdown,
                    window,
                    cx,
                )
            });
            cx.new(|cx| Root::new(editor, window, cx))
        }) {
            Ok(handle) => {
                self.note_editor_window = Some(handle.into());
                cx.notify();
            }
            Err(err) => {
                crate::debug_log!("[note] failed to open markdown editor window: {}", err);
                self.on_markdown_note_editor_window_closed(session_id, cx);
            }
        }
    }

    fn on_markdown_note_editor_window_closed(
        &mut self,
        session_id: u64,
        cx: &mut Context<Self>,
    ) {
        if self.note_editor_session != session_id {
            return;
        }
        let mut changed = false;
        if self.note_editor_open {
            self.note_editor_open = false;
            changed = true;
        }
        if self.note_editor_anchor.take().is_some() {
            changed = true;
        }
        if self.note_editor_edit_note_id.take().is_some() {
            changed = true;
        }
        if self.note_editor_window.take().is_some() {
            changed = true;
        }
        if changed {
            self.needs_root_refocus = true;
            cx.notify();
        }
    }

    fn close_markdown_note_editor(&mut self, cx: &mut Context<Self>) {
        let window_handle = self.note_editor_window.take();
        let mut changed = false;
        if self.note_editor_open {
            self.note_editor_open = false;
            changed = true;
        }
        if self.note_editor_anchor.take().is_some() {
            changed = true;
        }
        if self.note_editor_edit_note_id.take().is_some() {
            changed = true;
        }
        if changed || window_handle.is_some() {
            self.needs_root_refocus = true;
            cx.notify();
        }
        if let Some(window_handle) = window_handle {
            let _ = window_handle.update(cx, |_, window, _| {
                window.remove_window();
            });
        }
    }

    fn save_markdown_note_from_editor_window(
        &mut self,
        session_id: u64,
        markdown: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.note_editor_open || self.note_editor_session != session_id {
            return true;
        }
        let markdown = markdown.trim().to_string();
        if markdown.is_empty() {
            return false;
        }

        let now = Self::now_unix_secs();
        if let Some(note_id) = self.note_editor_edit_note_id
            && let Some(mut note) = self.markdown_note_by_id(note_id)
        {
            note.markdown = markdown;
            note.updated_at_unix_secs = now;
            self.upsert_markdown_note(note);
            self.on_markdown_note_editor_window_closed(session_id, cx);
            return true;
        }

        let Some(path) = self.active_tab_path().cloned() else {
            return false;
        };
        let Some(anchor) = self.note_editor_anchor else {
            return false;
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
        self.on_markdown_note_editor_window_closed(session_id, cx);
        true
    }
}
