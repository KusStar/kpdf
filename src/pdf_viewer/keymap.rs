use crate::pdf_viewer::PdfViewer;
use gpui::*;
use gpui_component::kbd::Kbd;
use gpui_component::*;

pub(super) struct KeymapWindow {
    _viewer: Entity<PdfViewer>,
    keymap_scroll: ScrollHandle,
    focus_handle: FocusHandle,
}

impl KeymapWindow {
    pub(super) fn new(viewer: Entity<PdfViewer>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            _viewer: viewer,
            keymap_scroll: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self._viewer.update(cx, |viewer, cx| {
            viewer.close_keymap_dialog(cx);
        });
        window.remove_window();
    }

    fn keymap_sections() -> Vec<(&'static str, Vec<(&'static str, Vec<Keystroke>)>)> {
        vec![
            (
                "Command Panel",
                vec![
                    ("command_panel_toggle", vec![Keystroke::parse("cmd-t").unwrap()]),
                    ("show_keymap", vec![Keystroke::parse("cmd-/").unwrap()]),
                ],
            ),
            (
                "File & Tabs",
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
                "Sidebar & Thumbnail",
                vec![
                    ("toggle_sidebar", vec![Keystroke::parse("cmd-b").unwrap()]),
                    ("toggle_thumbnail_panel", vec![Keystroke::parse("cmd-shift-t").unwrap()]),
                ],
            ),
            (
                "Zoom",
                vec![
                    ("zoom_in", vec![Keystroke::parse("cmd-=").unwrap()]),
                    ("zoom_out", vec![Keystroke::parse("cmd--").unwrap()]),
                    ("zoom_reset", vec![Keystroke::parse("cmd-0").unwrap()]),
                ],
            ),
            (
                "Page Navigation",
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
                "Text Selection",
                vec![
                    ("copy", vec![Keystroke::parse("cmd-c").unwrap()]),
                    ("select_all", vec![Keystroke::parse("cmd-a").unwrap()]),
                    ("clear_selection", vec![Keystroke::parse("escape").unwrap()]),
                ],
            ),
            (
                "Panels",
                vec![
                    ("toggle_bookmarks", vec![Keystroke::parse("cmd-shift-b").unwrap()]),
                    ("toggle_recent_files", vec![Keystroke::parse("cmd-shift-r").unwrap()]),
                ],
            ),
        ]
    }

    fn action_label(action: &str) -> String {
        match action {
            "command_panel_toggle" => "Toggle Command Panel".to_string(),
            "show_keymap" => "Show Keyboard Shortcuts".to_string(),
            "open_file" => "Open File".to_string(),
            "close_tab" => "Close Current Tab".to_string(),
            "switch_to_next_tab" => "Switch to Next Tab".to_string(),
            "switch_to_previous_tab" => "Switch to Previous Tab".to_string(),
            "switch_to_tab_1" => "Switch to Tab 1".to_string(),
            "switch_to_tab_2" => "Switch to Tab 2".to_string(),
            "switch_to_tab_3" => "Switch to Tab 3".to_string(),
            "switch_to_tab_4" => "Switch to Tab 4".to_string(),
            "switch_to_tab_5" => "Switch to Tab 5".to_string(),
            "switch_to_tab_6" => "Switch to Tab 6".to_string(),
            "switch_to_tab_7" => "Switch to Tab 7".to_string(),
            "switch_to_tab_8" => "Switch to Tab 8".to_string(),
            "switch_to_last_tab" => "Switch to Last Tab".to_string(),
            "toggle_sidebar" => "Toggle Sidebar (Auto Hide)".to_string(),
            "toggle_thumbnail_panel" => "Toggle Thumbnail Panel".to_string(),
            "zoom_in" => "Zoom In".to_string(),
            "zoom_out" => "Zoom Out".to_string(),
            "zoom_reset" => "Reset Zoom".to_string(),
            "previous_page" => "Previous Page".to_string(),
            "next_page" => "Next Page".to_string(),
            "first_page" => "First Page".to_string(),
            "last_page" => "Last Page".to_string(),
            "copy" => "Copy Selected Text".to_string(),
            "select_all" => "Select All on Page".to_string(),
            "clear_selection" => "Clear Selection".to_string(),
            "toggle_bookmarks" => "Toggle Bookmarks Panel".to_string(),
            "toggle_recent_files" => "Toggle Recent Files Panel".to_string(),
            _ => action.to_string(),
        }
    }
}

impl Render for KeymapWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = Self::keymap_sections();

        window.set_window_title("Keyboard Shortcuts kPDF");

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
                                    .child("Keyboard Shortcuts"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Quick reference for common keyboard shortcuts"),
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
                                            Self::render_keymap_section(title, &entries, cx)
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
        cx: &mut Context<Self>,
    ) -> Div {
        let title_label = title.to_string();
        let items: Vec<_> = entries
            .iter()
            .map(|(action, keystrokes)| {
                let label = Self::action_label(action);
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
            let dialog = cx.new(|cx| KeymapWindow::new(viewer, window, cx));
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
        if let Some(window_handle) = window_handle {
            let _ = window_handle.update(cx, |_, window, _| {
                window.remove_window();
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
