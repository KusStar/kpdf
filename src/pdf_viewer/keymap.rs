use crate::pdf_viewer::PdfViewer;
use crate::i18n::I18n;
use gpui::*;
use gpui_component::kbd::Kbd;
use gpui_component::*;

pub(super) struct KeymapWindow {
    _viewer: Entity<PdfViewer>,
    keymap_scroll: ScrollHandle,
    focus_handle: FocusHandle,
    i18n: I18n,
}

impl KeymapWindow {
    pub(super) fn new(viewer: Entity<PdfViewer>, i18n: I18n, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            _viewer: viewer,
            keymap_scroll: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            i18n,
        }
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self._viewer.update(cx, |viewer, cx| {
            viewer.close_keymap_dialog(cx);
        });
        window.remove_window();
    }

    fn keymap_sections(i18n: I18n) -> Vec<(&'static str, Vec<(&'static str, Vec<Keystroke>)>)> {
        vec![
            (
                &i18n.keymap_section_command_panel,
                vec![
                    ("command_panel_toggle", vec![Keystroke::parse("cmd-t").unwrap()]),
                    ("show_keymap", vec![Keystroke::parse("cmd-/").unwrap()]),
                ],
            ),
            (
                &i18n.keymap_section_file_and_tabs,
                vec![
                    ("open_file", vec![Keystroke::parse("cmd-o").unwrap()]),
                    ("close_tab", vec![Keystroke::parse("cmd-w").unwrap()]),
                    ("switch_to_next_tab", vec![Keystroke::parse("cmd-shift-]").unwrap()]),
                    ("switch_to_previous_tab", vec![Keystroke::parse("cmd-shift-[").unwrap()]),
                    ("switch_to_tab_1", vec![Keystroke::parse("cmd-1").unwrap()]),
                    ("switch_to_tab_2", vec![Keystroke::parse("cmd-2").unwrap()]),
                    ("switch_to_tab_3", vec![Keystroke::parse("cmd-3").unwrap()]),
                    ("switch_to_tab_4", vec![Keystroke::parse("cmd-4").unwrap()]),
                    ("switch_to_tab_5", vec![Keystroke::parse("cmd-5").unwrap()]),
                    ("switch_to_tab_6", vec![Keystroke::parse("cmd-6").unwrap()]),
                    ("switch_to_tab_7", vec![Keystroke::parse("cmd-7").unwrap()]),
                    ("switch_to_tab_8", vec![Keystroke::parse("cmd-8").unwrap()]),
                    ("switch_to_last_tab", vec![Keystroke::parse("cmd-9").unwrap()]),
                ],
            ),
            (
                &i18n.keymap_section_sidebar_and_thumbnail,
                vec![
                    ("toggle_sidebar", vec![Keystroke::parse("cmd-b").unwrap()]),
                    ("toggle_thumbnail_panel", vec![Keystroke::parse("cmd-shift-t").unwrap()]),
                ],
            ),
            (
                &i18n.keymap_section_zoom,
                vec![
                    ("zoom_in", vec![Keystroke::parse("cmd-=").unwrap()]),
                    ("zoom_out", vec![Keystroke::parse("cmd--").unwrap()]),
                    ("zoom_reset", vec![Keystroke::parse("cmd-0").unwrap()]),
                ],
            ),
            (
                &i18n.keymap_section_page_navigation,
                vec![
                    ("previous_page", vec![
                        Keystroke::parse("cmd-left").unwrap(),
                        Keystroke::parse("pageup").unwrap(),
                    ]),
                    ("next_page", vec![
                        Keystroke::parse("cmd-right").unwrap(),
                        Keystroke::parse("pagedown").unwrap(),
                    ]),
                    ("first_page", vec![Keystroke::parse("home").unwrap()]),
                    ("last_page", vec![Keystroke::parse("end").unwrap()]),
                ],
            ),
            (
                &i18n.keymap_section_text_selection,
                vec![
                    ("copy", vec![Keystroke::parse("cmd-c").unwrap()]),
                    ("select_all", vec![Keystroke::parse("cmd-a").unwrap()]),
                    ("clear_selection", vec![Keystroke::parse("escape").unwrap()]),
                ],
            ),
            (
                &i18n.keymap_section_panels,
                vec![
                    ("toggle_bookmarks", vec![Keystroke::parse("cmd-shift-b").unwrap()]),
                    ("toggle_recent_files", vec![Keystroke::parse("cmd-shift-r").unwrap()]),
                ],
            ),
        ]
    }

    fn action_label(action: &str, i18n: I18n) -> String {
        match action {
            "command_panel_toggle" => i18n.command_panel_toggle.to_string(),
            "show_keymap" => i18n.command_panel_show_keymap.to_string(),
            "open_file" => i18n.action_open_file.to_string(),
            "close_tab" => i18n.action_close_tab.to_string(),
            "switch_to_next_tab" => i18n.action_switch_to_next_tab.to_string(),
            "switch_to_previous_tab" => i18n.action_switch_to_previous_tab.to_string(),
            "switch_to_tab_1" => i18n.action_switch_to_tab_1.to_string(),
            "switch_to_tab_2" => i18n.action_switch_to_tab_2.to_string(),
            "switch_to_tab_3" => i18n.action_switch_to_tab_3.to_string(),
            "switch_to_tab_4" => i18n.action_switch_to_tab_4.to_string(),
            "switch_to_tab_5" => i18n.action_switch_to_tab_5.to_string(),
            "switch_to_tab_6" => i18n.action_switch_to_tab_6.to_string(),
            "switch_to_tab_7" => i18n.action_switch_to_tab_7.to_string(),
            "switch_to_tab_8" => i18n.action_switch_to_tab_8.to_string(),
            "switch_to_last_tab" => i18n.action_switch_to_last_tab.to_string(),
            "toggle_sidebar" => i18n.action_toggle_sidebar.to_string(),
            "toggle_thumbnail_panel" => i18n.action_toggle_thumbnail_panel.to_string(),
            "zoom_in" => i18n.action_zoom_in.to_string(),
            "zoom_out" => i18n.action_zoom_out.to_string(),
            "zoom_reset" => i18n.action_zoom_reset.to_string(),
            "previous_page" => i18n.action_previous_page.to_string(),
            "next_page" => i18n.action_next_page.to_string(),
            "first_page" => i18n.action_first_page.to_string(),
            "last_page" => i18n.action_last_page.to_string(),
            "copy" => i18n.action_copy.to_string(),
            "select_all" => i18n.action_select_all.to_string(),
            "clear_selection" => i18n.action_clear_selection.to_string(),
            "toggle_bookmarks" => i18n.action_toggle_bookmarks.to_string(),
            "toggle_recent_files" => i18n.action_toggle_recent_files.to_string(),
            _ => action.to_string(),
        }
    }
}

impl Render for KeymapWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = Self::keymap_sections(self.i18n);

        window.set_window_title(&format!("{} - kPDF", self.i18n.keymap_dialog_title));

        div()
            .id("keymap-window")
            .size_full()
            .v_flex()
            .bg(cx.theme().background)
            .focusable()
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(
                |this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key.as_str() == "escape" {
                        this.close_dialog(window, cx);
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
                    .p_4()
                    .gap_4()
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .text_color(cx.theme().foreground)
                                    .child(self.i18n.keymap_dialog_title.to_string()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.i18n.keymap_dialog_hint.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .id("keymap-list-scroll-wrap")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .track_scroll(&self.keymap_scroll)
                            .child(
                                div()
                                    .v_flex()
                                    .gap_4()
                                    .children(
                                        sections.into_iter().map(|(title, entries)| {
                                            Self::render_keymap_section(title, &entries, self.i18n, cx)
                                        }),
                                    ),
                            ),
                    ),
            )
    }
}

impl KeymapWindow {
    fn render_keymap_section(
        title: &str,
        entries: &[(&'static str, Vec<Keystroke>)],
        i18n: I18n,
        cx: &mut Context<Self>,
    ) -> Div {
        let title_label = title.to_string();
        let items: Vec<_> = entries
            .iter()
            .map(|(action, keystrokes)| {
                let label = Self::action_label(action, i18n);
                let kbd_items: Vec<_> = keystrokes
                    .iter()
                    .map(|keystroke| Kbd::new(keystroke.clone()))
                    .collect();

                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py_1()
                    .px_2()
                    .rounded_md()
                    .hover(|style| style.bg(cx.theme().muted))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child(label),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .children(kbd_items),
                    )
            })
            .collect();

        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child(title_label),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .children(items),
            )
    }
}

impl PdfViewer {
    pub(super) fn open_keymap_dialog(&mut self, cx: &mut Context<Self>) {
        if self.command_panel_open {
            self.close_command_panel(cx);
        }
        if self.recent_popup_open {
            self.close_recent_popup(cx);
        }
        if self.bookmark_popup_open {
            self.close_bookmark_popup(cx);
        }
        self.close_settings_dialog(cx);
        self.close_about_dialog(cx);
        if self.note_editor_open {
            self.close_markdown_note_editor(cx);
        }

        if self.keymap_dialog_open {
            if let Some(handle) = self.keymap_dialog_window.as_ref() {
                let _ = handle.update(cx, |_, window, _| {
                    window.activate_window();
                });
            }
            return;
        }

        self.keymap_dialog_open = true;
        self.needs_root_refocus = false;
        self.keymap_dialog_session = self.keymap_dialog_session.wrapping_add(1);
        let session_id = self.keymap_dialog_session;

        let i18n = self.i18n();
        let viewer = cx.entity();
        let viewer_for_close = viewer.clone();
        let window_options = WindowOptions {
            titlebar: Some(Self::dialog_titlebar_options()),
            window_bounds: Some(WindowBounds::centered(
                size(px(560.), px(720.)),
                cx,
            )),
            window_decorations: Some(WindowDecorations::Client),
            ..WindowOptions::default()
        };

        match cx.open_window(window_options, move |window, cx| {
            window.on_window_should_close(cx, move |_, cx| {
                let _ = viewer_for_close.update(cx, |this, cx| {
                    this.on_keymap_dialog_window_closed(session_id, cx);
                });
                true
            });
            let dialog = cx.new(|cx| KeymapWindow::new(viewer, i18n, window, cx));
            let dialog_focus = dialog.read(cx).focus_handle.clone();
            let root = cx.new(|cx| Root::new(dialog, window, cx));
            window.focus(&dialog_focus);
            root
        }) {
            Ok(handle) => {
                self.keymap_dialog_window = Some(handle.into());
                cx.notify();
            }
            Err(err) => {
                crate::debug_log!("[keymap] failed to open keymap window: {}", err);
                self.on_keymap_dialog_window_closed(session_id, cx);
            }
        }
    }

    pub(super) fn close_keymap_dialog(&mut self, cx: &mut Context<Self>) {
        let window_handle = self.keymap_dialog_window.take();
        let mut changed = false;
        if self.keymap_dialog_open {
            self.keymap_dialog_open = false;
            changed = true;
        }
        if changed || window_handle.is_some() {
            self.needs_root_refocus = true;
            cx.notify();
        }
        // Defer window removal to avoid borrow conflicts during event handling
        if let Some(window_handle) = window_handle {
            cx.defer(move |cx| {
                let _ = window_handle.update(cx, |_, window, _| {
                    window.remove_window();
                });
            });
        }
    }

    fn on_keymap_dialog_window_closed(&mut self, session_id: u64, cx: &mut Context<Self>) {
        if self.keymap_dialog_session == session_id {
            self.keymap_dialog_window = None;
            self.keymap_dialog_open = false;
            self.needs_root_refocus = true;
            cx.notify();
        }
    }
}
