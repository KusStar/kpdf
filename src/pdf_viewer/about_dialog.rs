impl PdfViewer {
    fn open_about_dialog(&mut self, cx: &mut Context<Self>) {
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
        if self.note_editor_open {
            self.close_markdown_note_editor(cx);
        }

        if self.about_dialog_open {
            if let Some(handle) = self.about_dialog_window.as_ref() {
                let _ = handle.update(cx, |_, window, _| {
                    window.activate_window();
                });
            }
            return;
        }

        self.about_dialog_open = true;
        self.needs_root_refocus = false;
        self.about_dialog_session = self.about_dialog_session.wrapping_add(1);
        let session_id = self.about_dialog_session;

        let initial_snapshot = AboutDialogSnapshot::from_viewer(self);
        let viewer = cx.entity();
        let viewer_for_close = viewer.clone();
        let window_options = WindowOptions {
            titlebar: Some(Self::dialog_titlebar_options()),
            window_bounds: Some(WindowBounds::centered(
                size(px(ABOUT_DIALOG_WIDTH + 80.), px(ABOUT_DIALOG_WINDOW_HEIGHT)),
                cx,
            )),
            window_decorations: Some(WindowDecorations::Client),
            ..WindowOptions::default()
        };
        let initial_snapshot_for_window = initial_snapshot.clone();

        match cx.open_window(window_options, move |window, cx| {
            window.on_window_should_close(cx, move |_, cx| {
                let _ = viewer_for_close.update(cx, |this, cx| {
                    this.on_about_dialog_window_closed(session_id, cx);
                });
                true
            });
            let dialog = cx.new(|cx| AboutDialogWindow::new(viewer, initial_snapshot_for_window, window, cx));
            let dialog_focus = dialog.read(cx).focus_handle.clone();
            let root = cx.new(|cx| Root::new(dialog, window, cx));
            window.focus(&dialog_focus);
            root
        }) {
            Ok(handle) => {
                self.about_dialog_window = Some(handle.into());
                cx.notify();
            }
            Err(err) => {
                crate::debug_log!("[about] failed to open about window: {}", err);
                self.on_about_dialog_window_closed(session_id, cx);
            }
        }
    }

    fn close_about_dialog(&mut self, cx: &mut Context<Self>) {
        let window_handle = self.about_dialog_window.take();
        let mut changed = false;
        if self.about_dialog_open {
            self.about_dialog_open = false;
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

    fn on_about_dialog_window_closed(&mut self, session_id: u64, cx: &mut Context<Self>) {
        if self.about_dialog_session != session_id {
            return;
        }
        let mut changed = false;
        if self.about_dialog_open {
            self.about_dialog_open = false;
            changed = true;
        }
        if self.about_dialog_window.take().is_some() {
            changed = true;
        }
        if changed {
            self.needs_root_refocus = true;
            cx.notify();
        }
    }

}

#[derive(Clone)]
struct AboutDialogSnapshot {
    language: Language,
    updater_status: String,
    updater_download_url: Option<String>,
    updater_is_checking: bool,
}

impl AboutDialogSnapshot {
    fn from_viewer(viewer: &PdfViewer) -> Self {
        let updater_download_url = match &viewer.updater_state {
            UpdaterUiState::Available { download_url, .. } => Some(download_url.clone()),
            _ => None,
        };
        Self {
            language: viewer.language,
            updater_status: viewer.updater_status_text(),
            updater_download_url,
            updater_is_checking: matches!(viewer.updater_state, UpdaterUiState::Checking),
        }
    }
}

struct AboutDialogWindow {
    viewer: Entity<PdfViewer>,
    snapshot: AboutDialogSnapshot,
    _viewer_observation: Subscription,
    focus_handle: FocusHandle,
}

impl AboutDialogWindow {
    fn new(viewer: Entity<PdfViewer>, snapshot: AboutDialogSnapshot, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let viewer_for_observe = viewer.clone();
        let viewer_observation = cx.observe(&viewer_for_observe, |this, viewer, cx| {
            this.snapshot = {
                let viewer = viewer.read(cx);
                AboutDialogSnapshot::from_viewer(&viewer)
            };
            cx.notify();
        });
        Self {
            viewer,
            snapshot,
            _viewer_observation: viewer_observation,
            focus_handle: cx.focus_handle(),
        }
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.viewer.update(cx, |viewer, cx| {
            viewer.close_about_dialog(cx);
        });
        window.remove_window();
    }
}

impl Render for AboutDialogWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = I18n::new(self.snapshot.language);
        let updater_status = self.snapshot.updater_status.clone();
        let updater_download_url = self.snapshot.updater_download_url.clone();
        let updater_is_checking = self.snapshot.updater_is_checking;

        let version = env!("CARGO_PKG_VERSION");
        window.set_window_title(&format!("{} kPDF", i18n.about_dialog_title));

        div()
            .id("about-dialog-window")
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
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .text_color(cx.theme().foreground)
                                    .child(format!("{} kPDF", i18n.about_dialog_title)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(i18n.about_app_info),
                            ),
                    )
                    .child(div().h(px(1.)).bg(cx.theme().border))
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(i18n.version_label),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child(version),
                                    ),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .items_start()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(i18n.website_label),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child(APP_REPOSITORY_URL),
                                    ),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .items_start()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(i18n.updates_label),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .whitespace_normal()
                                            .child(updater_status),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("about-open-website-window")
                                    .small()
                                    .label(i18n.open_website_button)
                                    .on_click(|_, _, cx| {
                                        cx.open_url(APP_REPOSITORY_URL);
                                    }),
                            )
                            .child(
                                Button::new("about-check-updates-window")
                                    .small()
                                    .ghost()
                                    .label(i18n.check_updates_button)
                                    .disabled(updater_is_checking)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let _ = this.viewer.update(cx, |viewer, cx| {
                                            viewer.check_for_updates(cx);
                                        });
                                    })),
                            )
                            .when_some(updater_download_url, |this, download_url| {
                                this.child(
                                    Button::new("about-download-update-window")
                                        .small()
                                        .label(i18n.download_update_button)
                                        .on_click(move |_, _, cx| {
                                            cx.open_url(download_url.as_str());
                                        }),
                                )
                            })
                            .child(
                                Button::new("about-close-window")
                                    .small()
                                    .ghost()
                                    .label(i18n.close_button)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_dialog(window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
