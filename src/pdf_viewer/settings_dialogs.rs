impl PdfViewer {
    fn format_storage_size(bytes: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0usize;
        while size >= 1024.0 && unit_index < UNITS.len().saturating_sub(1) {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{bytes} {}", UNITS[unit_index])
        } else {
            format!("{size:.1} {}", UNITS[unit_index])
        }
    }

    fn refresh_db_usage(&mut self, cx: &mut Context<Self>) {
        if self.db_usage_refreshing {
            return;
        }
        self.db_usage_refreshing = true;
        let db_path = self.db_path.clone();
        cx.notify();

        cx.spawn(async move |view, cx| {
            let usage_bytes = cx
                .background_executor()
                .spawn(async move { Self::directory_usage_bytes(&db_path) })
                .await;

            let _ = view.update(cx, |this, cx| {
                this.db_usage_bytes = usage_bytes;
                this.db_usage_refreshing = false;
                cx.notify();
            });
        })
        .detach();
    }

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
        if self.command_panel_open {
            self.close_command_panel(cx);
        }
        if self.recent_popup_open {
            self.close_recent_popup(cx);
        }
        if self.bookmark_popup_open {
            self.close_bookmark_popup(cx);
        }
        self.close_about_dialog(cx);
        if self.note_editor_open {
            self.close_markdown_note_editor(cx);
        }

        if self.settings_dialog_open {
            if let Some(handle) = self.settings_dialog_window.as_ref() {
                let _ = handle.update(cx, |_, window, _| {
                    window.activate_window();
                });
            }
            return;
        }

        self.settings_dialog_open = true;
        self.needs_root_refocus = false;
        self.settings_dialog_session = self.settings_dialog_session.wrapping_add(1);
        let session_id = self.settings_dialog_session;

        self.sync_theme_color_select(window, cx);
        self.refresh_db_usage(cx);

        let initial_snapshot = SettingsDialogSnapshot::from_viewer(self);
        let theme_color_select_state = self.theme_color_select_state.clone();
        let viewer = cx.entity();
        let viewer_for_close = viewer.clone();
        let window_options = WindowOptions {
            titlebar: Some(Self::dialog_titlebar_options()),
            window_bounds: Some(WindowBounds::centered(
                size(px(SETTINGS_DIALOG_WIDTH + 96.), px(SETTINGS_DIALOG_WINDOW_HEIGHT)),
                cx,
            )),
            window_decorations: Some(WindowDecorations::Client),
            ..WindowOptions::default()
        };
        let initial_snapshot_for_window = initial_snapshot.clone();
        let theme_color_select_state_for_window = theme_color_select_state.clone();

        match cx.open_window(window_options, move |window, cx| {
            window.on_window_should_close(cx, move |_, cx| {
                let _ = viewer_for_close.update(cx, |this, cx| {
                    this.on_settings_dialog_window_closed(session_id, cx);
                });
                true
            });
            let dialog = cx.new(|cx| {
                SettingsDialogWindow::new(
                    viewer,
                    theme_color_select_state_for_window,
                    initial_snapshot_for_window,
                    cx,
                )
            });
            cx.new(|cx| Root::new(dialog, window, cx))
        }) {
            Ok(handle) => {
                self.settings_dialog_window = Some(handle.into());
                cx.notify();
            }
            Err(err) => {
                crate::debug_log!("[settings] failed to open settings window: {}", err);
                self.on_settings_dialog_window_closed(session_id, cx);
            }
        }
    }

    fn close_settings_dialog(&mut self, cx: &mut Context<Self>) {
        let window_handle = self.settings_dialog_window.take();
        let mut changed = false;
        if self.settings_dialog_open {
            self.settings_dialog_open = false;
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

    fn on_settings_dialog_window_closed(&mut self, session_id: u64, cx: &mut Context<Self>) {
        if self.settings_dialog_session != session_id {
            return;
        }
        let mut changed = false;
        if self.settings_dialog_open {
            self.settings_dialog_open = false;
            changed = true;
        }
        if self.settings_dialog_window.take().is_some() {
            changed = true;
        }
        if changed {
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

    fn set_language_preference(
        &mut self,
        preference: LanguagePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.language_preference == preference {
            return;
        }

        self.language_preference = preference;
        self.language = Self::resolve_language(preference, Language::detect());
        self.persist_language_preference();

        let i18n = self.i18n();
        self.command_panel_input_state
            .update(cx, |input, cx| input.set_placeholder(i18n.command_panel_search_hint, window, cx));
        crate::configure_app_menus(cx, i18n);
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

    #[allow(dead_code)]
    fn render_settings_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.settings_dialog_open {
            return None;
        }

        let i18n = self.i18n();
        let has_theme_color_options =
            !Self::available_theme_names_for_mode(self.theme_mode, cx).is_empty();
        let db_usage_text = Self::format_storage_size(self.db_usage_bytes);
        let db_path_text = self.db_path.to_string_lossy().to_string();
        let refresh_db_label: SharedString = if self.db_usage_refreshing {
            "…".into()
        } else {
            i18n.settings_db_refresh_button.into()
        };

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
                                                .child(i18n.settings_language_section),
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
                                                                            i18n.settings_language_label,
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
                                                                            i18n.settings_language_hint,
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            ButtonGroup::new("settings-language-preference")
                                                                .small()
                                                                .outline()
                                                                .child(
                                                                    Button::new("settings-language-system")
                                                                        .label(i18n.settings_language_system)
                                                                        .selected(
                                                                            self.language_preference
                                                                                == LanguagePreference::System,
                                                                        ),
                                                                )
                                                                .child(
                                                                    Button::new("settings-language-zh-cn")
                                                                        .label(i18n.settings_language_zh_cn)
                                                                        .selected(
                                                                            self.language_preference
                                                                                == LanguagePreference::ZhCn,
                                                                        ),
                                                                )
                                                                .child(
                                                                    Button::new("settings-language-en-us")
                                                                        .label(i18n.settings_language_en_us)
                                                                        .selected(
                                                                            self.language_preference
                                                                                == LanguagePreference::EnUs,
                                                                        ),
                                                                )
                                                                .on_click(cx.listener(
                                                                    |this, selected: &Vec<usize>, window, cx| {
                                                                        let preference = match selected.first().copied() {
                                                                            Some(1) => LanguagePreference::ZhCn,
                                                                            Some(2) => LanguagePreference::EnUs,
                                                                            _ => LanguagePreference::System,
                                                                        };
                                                                        this.set_language_preference(
                                                                            preference,
                                                                            window,
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
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(i18n.settings_db_section),
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
                                                                .child(i18n.settings_db_usage_label),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(cx.theme().muted_foreground)
                                                                .whitespace_normal()
                                                                .child(i18n.settings_db_usage_hint),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(cx.theme().muted_foreground)
                                                                .whitespace_normal()
                                                                .child(format!(
                                                                    "{}: {}",
                                                                    i18n.settings_db_path_label, db_path_text
                                                                )),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .v_flex()
                                                        .items_end()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(cx.theme().foreground)
                                                                .child(db_usage_text),
                                                        )
                                                        .child(
                                                            Button::new("settings-db-refresh")
                                                                .small()
                                                                .ghost()
                                                                .label(refresh_db_label)
                                                                .disabled(self.db_usage_refreshing)
                                                                .on_click(cx.listener(
                                                                    |this, _, _, cx| {
                                                                        this.refresh_db_usage(cx);
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


}

#[derive(Clone)]
struct SettingsDialogSnapshot {
    language: Language,
    language_preference: LanguagePreference,
    theme_mode: ThemeMode,
    titlebar_preferences: TitleBarVisibilityPreferences,
    db_usage_refreshing: bool,
    db_usage_bytes: u64,
    db_path_text: String,
}

impl SettingsDialogSnapshot {
    fn from_viewer(viewer: &PdfViewer) -> Self {
        Self {
            language: viewer.language,
            language_preference: viewer.language_preference,
            theme_mode: viewer.theme_mode,
            titlebar_preferences: viewer.titlebar_preferences,
            db_usage_refreshing: viewer.db_usage_refreshing,
            db_usage_bytes: viewer.db_usage_bytes,
            db_path_text: viewer.db_path.to_string_lossy().to_string(),
        }
    }
}


struct SettingsDialogWindow {
    viewer: Entity<PdfViewer>,
    theme_color_select_state: Entity<SelectState<SearchableVec<SharedString>>>,
    snapshot: SettingsDialogSnapshot,
    _viewer_observation: Subscription,
}

impl SettingsDialogWindow {
    fn new(
        viewer: Entity<PdfViewer>,
        theme_color_select_state: Entity<SelectState<SearchableVec<SharedString>>>,
        snapshot: SettingsDialogSnapshot,
        cx: &mut Context<Self>,
    ) -> Self {
        let viewer_for_observe = viewer.clone();
        let viewer_observation = cx.observe(&viewer_for_observe, |this, viewer, cx| {
            this.snapshot = {
                let viewer = viewer.read(cx);
                SettingsDialogSnapshot::from_viewer(&viewer)
            };
            cx.notify();
        });
        Self {
            viewer,
            theme_color_select_state,
            snapshot,
            _viewer_observation: viewer_observation,
        }
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        let _ = self.viewer.update(cx, |viewer, cx| {
            viewer.close_settings_dialog(cx);
        });
    }
}

impl Render for SettingsDialogWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = I18n::new(self.snapshot.language);
        let theme_mode = self.snapshot.theme_mode;
        let language_preference = self.snapshot.language_preference;
        let titlebar_preferences = self.snapshot.titlebar_preferences;
        let theme_color_select_state = self.theme_color_select_state.clone();
        let db_usage_refreshing = self.snapshot.db_usage_refreshing;
        let db_usage_bytes = self.snapshot.db_usage_bytes;
        let db_path_text = self.snapshot.db_path_text.clone();
        let has_theme_color_options = ThemeRegistry::global(cx)
            .sorted_themes()
            .into_iter()
            .any(|theme| theme.mode == theme_mode);
        let db_usage_text = PdfViewer::format_storage_size(db_usage_bytes);
        let refresh_db_label: SharedString = if db_usage_refreshing {
            "…".into()
        } else {
            i18n.settings_db_refresh_button.into()
        };

        window.set_window_title(i18n.settings_dialog_title);

        div()
            .size_full()
            .v_flex()
            .bg(cx.theme().background)
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
                                    .child(i18n.settings_language_section),
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
                                                            .child(i18n.settings_language_label),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .whitespace_normal()
                                                            .child(i18n.settings_language_hint),
                                                    ),
                                            )
                                            .child(
                                                ButtonGroup::new("settings-language-preference-window")
                                                    .small()
                                                    .outline()
                                                    .child(
                                                        Button::new("settings-language-system-window")
                                                            .label(i18n.settings_language_system)
                                                            .selected(
                                                                language_preference
                                                                    == LanguagePreference::System,
                                                            ),
                                                    )
                                                    .child(
                                                        Button::new("settings-language-zh-cn-window")
                                                            .label(i18n.settings_language_zh_cn)
                                                            .selected(
                                                                language_preference
                                                                    == LanguagePreference::ZhCn,
                                                            ),
                                                    )
                                                    .child(
                                                        Button::new("settings-language-en-us-window")
                                                            .label(i18n.settings_language_en_us)
                                                            .selected(
                                                                language_preference
                                                                    == LanguagePreference::EnUs,
                                                            ),
                                                    )
                                                    .on_click(cx.listener(
                                                        |this, selected: &Vec<usize>, window, cx| {
                                                            let preference =
                                                                match selected.first().copied() {
                                                                    Some(1) => LanguagePreference::ZhCn,
                                                                    Some(2) => LanguagePreference::EnUs,
                                                                    _ => LanguagePreference::System,
                                                                };
                                                            let _ =
                                                                this.viewer.update(cx, |viewer, cx| {
                                                                    viewer.set_language_preference(
                                                                        preference,
                                                                        window,
                                                                        cx,
                                                                    );
                                                                });
                                                        },
                                                    )),
                                            ),
                                    ),
                            ),
                    )
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
                                                            .child(i18n.settings_theme_label),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .whitespace_normal()
                                                            .child(i18n.settings_theme_hint),
                                                    ),
                                            )
                                            .child(
                                                ButtonGroup::new("settings-theme-mode-window")
                                                    .small()
                                                    .outline()
                                                    .child(
                                                        Button::new("settings-theme-light-window")
                                                            .label(i18n.settings_theme_light)
                                                            .selected(theme_mode == ThemeMode::Light),
                                                    )
                                                    .child(
                                                        Button::new("settings-theme-dark-window")
                                                            .label(i18n.settings_theme_dark)
                                                            .selected(theme_mode == ThemeMode::Dark),
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
                                                            let _ =
                                                                this.viewer.update(cx, |viewer, cx| {
                                                                    viewer.set_theme_mode(
                                                                        mode,
                                                                        window,
                                                                        cx,
                                                                    );
                                                                });
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
                                                            .child(i18n.settings_theme_color_label),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .whitespace_normal()
                                                            .child(i18n.settings_theme_color_hint),
                                                    ),
                                            )
                                            .child(
                                                div().w(px(200.)).child(
                                                    Select::new(&theme_color_select_state)
                                                        .small()
                                                        .disabled(!has_theme_color_options)
                                                        .placeholder(
                                                            i18n.settings_theme_color_placeholder,
                                                        ),
                                                ),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
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
                                                            .child(i18n.settings_titlebar_navigation_label),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .whitespace_normal()
                                                            .child(i18n.settings_titlebar_navigation_hint),
                                                    ),
                                            )
                                            .child(
                                                Checkbox::new(
                                                    "settings-show-titlebar-navigation-window",
                                                )
                                                .checked(titlebar_preferences.show_navigation)
                                                .on_click(cx.listener(
                                                    |this, checked: &bool, _, cx| {
                                                        let _ = this.viewer.update(cx, |viewer, cx| {
                                                            viewer.set_titlebar_navigation_visible(
                                                                *checked, cx,
                                                            );
                                                        });
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
                                                            .child(i18n.settings_titlebar_zoom_label),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .whitespace_normal()
                                                            .child(i18n.settings_titlebar_zoom_hint),
                                                    ),
                                            )
                                            .child(
                                                Checkbox::new("settings-show-titlebar-zoom-window")
                                                    .checked(titlebar_preferences.show_zoom)
                                                    .on_click(cx.listener(
                                                        |this, checked: &bool, _, cx| {
                                                            let _ = this.viewer.update(cx, |viewer, cx| {
                                                                viewer.set_titlebar_zoom_visible(*checked, cx);
                                                            });
                                                        },
                                                    )),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(i18n.settings_db_section),
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
                                                            .child(i18n.settings_db_usage_label),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .whitespace_normal()
                                                            .child(i18n.settings_db_usage_hint),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .whitespace_normal()
                                                            .child(format!(
                                                                "{}: {}",
                                                                i18n.settings_db_path_label, db_path_text
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .v_flex()
                                                    .items_end()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child(db_usage_text),
                                                    )
                                                    .child(
                                                        Button::new("settings-db-refresh-window")
                                                            .small()
                                                            .ghost()
                                                            .label(refresh_db_label)
                                                            .disabled(db_usage_refreshing)
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    let _ = this.viewer.update(cx, |viewer, cx| {
                                                                        viewer.refresh_db_usage(cx);
                                                                    });
                                                                },
                                                            )),
                                                    ),
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
                                Button::new("settings-close-window")
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
