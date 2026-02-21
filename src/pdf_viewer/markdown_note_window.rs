struct MarkdownNoteEditorWindow {
    viewer: Entity<PdfViewer>,
    session_id: u64,
    language: Language,
    is_editing: bool,
    input_state: Entity<InputState>,
    needs_focus: bool,
    preview_visible: bool,
}

impl MarkdownNoteEditorWindow {
    fn new(
        viewer: Entity<PdfViewer>,
        session_id: u64,
        language: Language,
        is_editing: bool,
        initial_markdown: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(14)
                .placeholder(I18n::new(language).note_input_placeholder)
        });
        input_state.update(cx, |input, cx| {
            input.set_value(initial_markdown, window, cx);
        });

        Self {
            viewer,
            session_id,
            language,
            is_editing,
            input_state,
            needs_focus: true,
            preview_visible: true,
        }
    }

    fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.viewer.update(cx, |viewer, cx| {
            viewer.on_markdown_note_editor_window_closed(self.session_id, cx);
        });
        window.remove_window();
    }

    fn save_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let markdown = self.input_state.read(cx).value().to_string();
        let should_close = self.viewer.update(cx, |viewer, cx| {
            viewer.save_markdown_note_from_editor_window(self.session_id, markdown, cx)
        });
        if should_close {
            window.remove_window();
        }
    }
}

impl Render for MarkdownNoteEditorWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_focus {
            self.needs_focus = false;
            let _ = self
                .input_state
                .update(cx, |input, cx| input.focus(window, cx));
        }

        let i18n = I18n::new(self.language);
        let title = if self.is_editing {
            i18n.note_edit_dialog_title
        } else {
            i18n.note_new_dialog_title
        };
        window.set_window_title(title);

        let editor_text = self.input_state.read(cx).value().to_string();
        let can_save = !editor_text.trim().is_empty();
        let preview_text = if editor_text.trim().is_empty() {
            i18n.note_input_placeholder.to_string()
        } else {
            editor_text.clone()
        };
        let preview_toggle_label = if self.preview_visible {
            i18n.note_hide_preview_button
        } else {
            i18n.note_show_preview_button
        };
        let preview_id: SharedString =
            format!("markdown-note-preview-window-{}", self.session_id).into();

        div()
            .id("markdown-note-editor-window")
            .size_full()
            .v_flex()
            .bg(cx.theme().background)
            .capture_key_down(cx.listener(
                |this, event: &KeyDownEvent, window, cx| {
                    let is_primary_modifier = event.keystroke.modifiers.secondary();
                    let key = event.keystroke.key.as_str();
                    if key == "escape" {
                        this.close_editor(window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    if key == "enter" && is_primary_modifier {
                        this.save_editor(window, cx);
                        cx.stop_propagation();
                    }
                },
            ))
            .child(TitleBar::new())
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .text_color(cx.theme().foreground)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .whitespace_normal()
                            .child(i18n.note_dialog_hint),
                    )
                    .child(
                        div().w_full().flex().justify_end().child(
                            Button::new("markdown-note-preview-toggle-window")
                                .small()
                                .ghost()
                                .label(preview_toggle_label)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.preview_visible = !this.preview_visible;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex_1()
                            .flex()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .child(Input::new(&self.input_state).h_full()),
                            )
                            .when(self.preview_visible, |this| {
                                this.child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .h_full()
                                        .v_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .w_full()
                                                .flex_1()
                                                .rounded_md()
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .bg(cx.theme().background)
                                                .overflow_hidden()
                                                .p_3()
                                                .child(
                                                    TextView::markdown(
                                                        preview_id,
                                                        preview_text,
                                                        window,
                                                        cx,
                                                    )
                                                    .selectable(true)
                                                    .scrollable(true),
                                                ),
                                        ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("markdown-note-cancel-window")
                                    .small()
                                    .ghost()
                                    .label(i18n.note_cancel_button)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_editor(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("markdown-note-save-window")
                                    .small()
                                    .label(i18n.note_save_button)
                                    .disabled(!can_save)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_editor(window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
