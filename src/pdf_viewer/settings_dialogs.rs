impl PdfViewer {
    fn summarize_updater_error(raw: &str) -> String {
        const MAX_LEN: usize = 88;
        let mut message = raw
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or(raw)
            .trim()
            .to_string();
        if message.is_empty() {
            message = "unknown error".to_string();
        }
        if message.len() > MAX_LEN {
            message.truncate(MAX_LEN - 3);
            message.push_str("...");
        }
        message
    }

    fn updater_status_text(&self) -> String {
        let i18n = self.i18n();
        match &self.updater_state {
            UpdaterUiState::Idle => i18n.update_status_idle.to_string(),
            UpdaterUiState::Checking => i18n.update_status_checking.to_string(),
            UpdaterUiState::UpToDate { latest_version } => {
                i18n.update_status_up_to_date(latest_version)
            }
            UpdaterUiState::Available { latest_version, .. } => {
                i18n.update_status_available(latest_version)
            }
            UpdaterUiState::Error { message } => i18n.update_status_failed(message),
        }
    }

    fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if matches!(self.updater_state, UpdaterUiState::Checking) {
            return;
        }

        self.updater_state = UpdaterUiState::Checking;
        cx.notify();

        cx.spawn(async move |view, cx| {
            let update_result = cx
                .background_executor()
                .spawn(async move { updater::check_for_updates(env!("CARGO_PKG_VERSION")) })
                .await;

            let _ = view.update(cx, |this, cx| {
                match update_result {
                    Ok(updater::UpdateCheck::UpToDate { latest_version }) => {
                        this.updater_state = UpdaterUiState::UpToDate { latest_version };
                    }
                    Ok(updater::UpdateCheck::UpdateAvailable(info)) => {
                        this.updater_state = UpdaterUiState::Available {
                            latest_version: info.latest_version,
                            download_url: info.download_url,
                        };
                    }
                    Err(err) => {
                        crate::debug_log!("[updater] check failed: {}", err);
                        this.updater_state = UpdaterUiState::Error {
                            message: Self::summarize_updater_error(&err.to_string()),
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        if self.command_panel_open {
            self.close_command_panel(cx);
            changed = true;
        }
        if self.recent_popup_open {
            self.close_recent_popup(cx);
            changed = true;
        }
        if self.bookmark_popup_open {
            self.close_bookmark_popup(cx);
            changed = true;
        }
        if self.about_dialog_open {
            self.about_dialog_open = false;
            changed = true;
        }
        if self.note_editor_open {
            self.close_markdown_note_editor(cx);
            changed = true;
        }
        if !self.settings_dialog_open {
            self.settings_dialog_open = true;
            changed = true;
        }
        self.sync_theme_color_select(window, cx);
        if changed {
            cx.notify();
        }
    }

    fn close_settings_dialog(&mut self, cx: &mut Context<Self>) {
        if self.settings_dialog_open {
            self.settings_dialog_open = false;
            self.needs_root_refocus = true;
            cx.notify();
        }
    }

    fn set_titlebar_navigation_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.titlebar_preferences.show_navigation == visible {
            return;
        }
        self.titlebar_preferences.show_navigation = visible;
        self.persist_titlebar_preferences();
        cx.notify();
    }

    fn active_theme_name_for_mode(&self, mode: ThemeMode, cx: &Context<Self>) -> SharedString {
        let theme = cx.theme();
        if mode == ThemeMode::Dark {
            theme.dark_theme.name.clone()
        } else {
            theme.light_theme.name.clone()
        }
    }

    fn available_theme_names_for_mode(mode: ThemeMode, cx: &Context<Self>) -> Vec<SharedString> {
        ThemeRegistry::global(cx)
            .sorted_themes()
            .into_iter()
            .filter(|theme| theme.mode == mode)
            .map(|theme| theme.name.clone())
            .collect()
    }

    fn apply_theme_preferences(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        let (selected_light_theme, selected_dark_theme) = {
            let registry = ThemeRegistry::global(cx);
            let themes = registry.themes();

            let selected_light_theme = self
                .preferred_light_theme_name
                .as_ref()
                .and_then(|name| themes.get(name.as_str()).cloned())
                .filter(|theme| theme.mode == ThemeMode::Light)
                .unwrap_or_else(|| registry.default_light_theme().clone());
            let selected_dark_theme = self
                .preferred_dark_theme_name
                .as_ref()
                .and_then(|name| themes.get(name.as_str()).cloned())
                .filter(|theme| theme.mode == ThemeMode::Dark)
                .unwrap_or_else(|| registry.default_dark_theme().clone());
            (selected_light_theme, selected_dark_theme)
        };

        {
            let theme = Theme::global_mut(cx);
            theme.light_theme = selected_light_theme;
            theme.dark_theme = selected_dark_theme;
        }

        if let Some(window) = window {
            Theme::change(self.theme_mode, Some(window), cx);
        } else {
            Theme::change(self.theme_mode, None, cx);
            cx.refresh_windows();
        }
    }

    fn sync_theme_color_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let theme_names = Self::available_theme_names_for_mode(self.theme_mode, cx);
        let active_theme_name = self.active_theme_name_for_mode(self.theme_mode, cx);
        let has_active_theme = theme_names
            .iter()
            .any(|name| name.as_ref() == active_theme_name.as_ref());

        self.theme_color_select_state.update(cx, |state, cx| {
            state.set_items(SearchableVec::new(theme_names.clone()), window, cx);
            if has_active_theme {
                state.set_selected_value(&active_theme_name, window, cx);
            } else {
                state.set_selected_index(None, window, cx);
            }
        });
    }

    fn set_theme_color_by_name(
        &mut self,
        mode: ThemeMode,
        theme_name: &str,
        cx: &mut Context<Self>,
    ) {
        if mode == ThemeMode::Dark {
            if self.preferred_dark_theme_name.as_deref() == Some(theme_name) {
                return;
            }
            self.preferred_dark_theme_name = Some(theme_name.to_string());
        } else {
            if self.preferred_light_theme_name.as_deref() == Some(theme_name) {
                return;
            }
            self.preferred_light_theme_name = Some(theme_name.to_string());
        }

        self.persist_theme_preferences();
        self.apply_theme_preferences(None, cx);
        cx.notify();
    }

    fn set_theme_mode(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_mode == mode {
            return;
        }
        self.theme_mode = mode;
        self.persist_theme_preferences();
        self.apply_theme_preferences(Some(window), cx);
        self.sync_theme_color_select(window, cx);
        cx.notify();
    }

    fn set_titlebar_zoom_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.titlebar_preferences.show_zoom == visible {
            return;
        }
        self.titlebar_preferences.show_zoom = visible;
        self.persist_titlebar_preferences();
        cx.notify();
    }

    fn render_settings_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.settings_dialog_open {
            return None;
        }

        let i18n = self.i18n();
        let has_theme_color_options =
            !Self::available_theme_names_for_mode(self.theme_mode, cx).is_empty();

        Some(
            div()
                .id("settings-dialog-overlay")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .bg(cx.theme().background.opacity(0.45))
                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                    cx.stop_propagation();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.close_settings_dialog(cx);
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .v_flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("settings-dialog")
                                .w(px(SETTINGS_DIALOG_WIDTH))
                                .v_flex()
                                .gap_3()
                                .popover_style(cx)
                                .p_4()
                                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                                    cx.stop_propagation();
                                }))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_, _, _, cx| {
                                        cx.stop_propagation();
                                    }),
                                )
                                .child(
                                    div()
                                        .text_lg()
                                        .text_color(cx.theme().foreground)
                                        .child(i18n.settings_dialog_title),
                                )
                                .child(div().h(px(1.)).bg(cx.theme().border))
                                .child(
                                    div()
                                        .v_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(i18n.settings_theme_section),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .rounded_md()
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .p_3()
                                                .v_flex()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .flex()
                                                        .items_start()
                                                        .justify_between()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .v_flex()
                                                                .items_start()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(cx.theme().foreground)
                                                                        .child(
                                                                            i18n.settings_theme_label,
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .whitespace_normal()
                                                                        .child(i18n.settings_theme_hint),
                                                                ),
                                                        )
                                                        .child(
                                                            ButtonGroup::new("settings-theme-mode")
                                                                .small()
                                                                .outline()
                                                                .child(
                                                                    Button::new("settings-theme-light")
                                                                        .label(i18n.settings_theme_light)
                                                                        .selected(
                                                                            self.theme_mode
                                                                                == ThemeMode::Light,
                                                                        ),
                                                                )
                                                                .child(
                                                                    Button::new("settings-theme-dark")
                                                                        .label(i18n.settings_theme_dark)
                                                                        .selected(
                                                                            self.theme_mode
                                                                                == ThemeMode::Dark,
                                                                        ),
                                                                )
                                                                .on_click(cx.listener(
                                                                    |this, selected: &Vec<usize>, window, cx| {
                                                                        let mode = if selected.first().copied()
                                                                            == Some(1)
                                                                        {
                                                                            ThemeMode::Dark
                                                                        } else {
                                                                            ThemeMode::Light
                                                                        };
                                                                        this.set_theme_mode(mode, window, cx);
                                                                    },
                                                                )),
                                                        ),
                                                )
                                                .child(div().h(px(1.)).bg(cx.theme().border))
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .flex()
                                                        .items_start()
                                                        .justify_between()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .v_flex()
                                                                .items_start()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(cx.theme().foreground)
                                                                        .child(
                                                                            i18n.settings_theme_color_label,
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme().muted_foreground,
                                                                        )
                                                                        .whitespace_normal()
                                                                        .child(
                                                                            i18n.settings_theme_color_hint,
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(200.))
                                                                .child(
                                                                    Select::new(
                                                                        &self.theme_color_select_state,
                                                                    )
                                                                    .small()
                                                                    .disabled(!has_theme_color_options)
                                                                    .placeholder(
                                                                        i18n.settings_theme_color_placeholder,
                                                                    ),
                                                                ),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(i18n.settings_titlebar_section),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .rounded_md()
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .p_3()
                                                .v_flex()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .flex()
                                                        .items_start()
                                                        .justify_between()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .v_flex()
                                                                .items_start()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(cx.theme().foreground)
                                                                        .child(
                                                                            i18n.settings_titlebar_navigation_label,
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .whitespace_normal()
                                                                        .child(
                                                                            i18n.settings_titlebar_navigation_hint,
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            Checkbox::new("settings-show-titlebar-navigation")
                                                                .checked(
                                                                    self.titlebar_preferences
                                                                        .show_navigation,
                                                                )
                                                                .on_click(cx.listener(
                                                                    |this, checked: &bool, _, cx| {
                                                                        this.set_titlebar_navigation_visible(
                                                                            *checked,
                                                                            cx,
                                                                        );
                                                                    },
                                                                )),
                                                        ),
                                                )
                                                .child(div().h(px(1.)).bg(cx.theme().border))
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .flex()
                                                        .items_start()
                                                        .justify_between()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .v_flex()
                                                                .items_start()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(cx.theme().foreground)
                                                                        .child(
                                                                            i18n.settings_titlebar_zoom_label,
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .whitespace_normal()
                                                                        .child(
                                                                            i18n.settings_titlebar_zoom_hint,
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            Checkbox::new("settings-show-titlebar-zoom")
                                                                .checked(self.titlebar_preferences.show_zoom)
                                                                .on_click(cx.listener(
                                                                    |this, checked: &bool, _, cx| {
                                                                        this.set_titlebar_zoom_visible(
                                                                            *checked,
                                                                            cx,
                                                                        );
                                                                    },
                                                                )),
                                                        ),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .child(
                                            Button::new("settings-close")
                                                .small()
                                                .ghost()
                                                .label(i18n.close_button)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.close_settings_dialog(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

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
        if self.settings_dialog_open {
            self.settings_dialog_open = false;
        }
        if self.note_editor_open {
            self.close_markdown_note_editor(cx);
        }
        if !self.about_dialog_open {
            self.about_dialog_open = true;
            cx.notify();
        }
    }

    fn close_about_dialog(&mut self, cx: &mut Context<Self>) {
        if self.about_dialog_open {
            self.about_dialog_open = false;
            self.needs_root_refocus = true;
            cx.notify();
        }
    }

    fn render_about_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.about_dialog_open {
            return None;
        }

        let i18n = self.i18n();
        let version = env!("CARGO_PKG_VERSION");
        let updater_status = self.updater_status_text();
        let updater_download_url = match &self.updater_state {
            UpdaterUiState::Available { download_url, .. } => Some(download_url.clone()),
            _ => None,
        };
        let updater_is_checking = matches!(self.updater_state, UpdaterUiState::Checking);

        Some(
            div()
                .id("about-dialog-overlay")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .bg(cx.theme().background.opacity(0.45))
                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                    cx.stop_propagation();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.close_about_dialog(cx);
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .v_flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("about-dialog")
                                .w(px(ABOUT_DIALOG_WIDTH))
                                .v_flex()
                                .gap_3()
                                .popover_style(cx)
                                .p_4()
                                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                                    cx.stop_propagation();
                                }))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_, _, _, cx| {
                                        cx.stop_propagation();
                                    }),
                                )
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
                                            Button::new("about-open-website")
                                                .small()
                                                .label(i18n.open_website_button)
                                                .on_click(|_, _, cx| {
                                                    cx.open_url(APP_REPOSITORY_URL);
                                                }),
                                        )
                                        .child(
                                            Button::new("about-check-updates")
                                                .small()
                                                .ghost()
                                                .label(i18n.check_updates_button)
                                                .disabled(updater_is_checking)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.check_for_updates(cx);
                                                })),
                                        )
                                        .when_some(updater_download_url, |this, download_url| {
                                            this.child(
                                                Button::new("about-download-update")
                                                    .small()
                                                    .label(i18n.download_update_button)
                                                    .on_click(move |_, _, cx| {
                                                        cx.open_url(download_url.as_str());
                                                    }),
                                            )
                                        })
                                        .child(
                                            Button::new("about-close")
                                                .small()
                                                .ghost()
                                                .label(i18n.close_button)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.close_about_dialog(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

}

struct MarkdownNoteEditorWindow {
    viewer: Entity<PdfViewer>,
    session_id: u64,
    language: Language,
    is_editing: bool,
    input_state: Entity<InputState>,
    needs_focus: bool,
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
                .placeholder(I18n::new(language).markdown_note_input_placeholder)
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
            i18n.markdown_note_edit_dialog_title
        } else {
            i18n.markdown_note_new_dialog_title
        };
        window.set_window_title(title);

        let editor_text = self.input_state.read(cx).value().to_string();
        let can_save = !editor_text.trim().is_empty();
        let preview_text = if editor_text.trim().is_empty() {
            i18n.markdown_note_input_placeholder.to_string()
        } else {
            editor_text.clone()
        };
        let preview_id: SharedString =
            format!("markdown-note-preview-window-{}", self.session_id).into();

        div()
            .id("markdown-note-editor-window")
            .size_full()
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
            .child(
                div()
                    .size_full()
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
                            .child(i18n.markdown_note_dialog_hint),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(MARKDOWN_NOTE_EDITOR_INPUT_HEIGHT))
                            .child(Input::new(&self.input_state).h_full()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(i18n.markdown_note_preview_label),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(220.))
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .overflow_hidden()
                            .p_3()
                            .child(
                                TextView::markdown(preview_id, preview_text, window, cx)
                                    .selectable(true)
                                    .scrollable(true),
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
                                Button::new("markdown-note-cancel-window")
                                    .small()
                                    .ghost()
                                    .label(i18n.markdown_note_cancel_button)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_editor(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("markdown-note-save-window")
                                    .small()
                                    .label(i18n.markdown_note_save_button)
                                    .disabled(!can_save)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_editor(window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
