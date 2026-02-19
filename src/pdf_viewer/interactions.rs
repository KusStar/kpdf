impl PdfViewer {
    pub fn copy_selected_text(&self) {
        if let Some(tab) = self.active_tab() {
            let manager = tab.text_selection_manager.borrow();
            if let Some(text) = manager.get_selected_text() {
                if !text.is_empty() {
                    if let Err(err) = copy_to_clipboard(&text) {
                        crate::debug_log!("[copy] failed to copy to clipboard: {}", err);
                    }
                }
            }
        }
    }

    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            if tab.pages.get(tab.active_page).is_some() {
                let mut manager = tab.text_selection_manager.borrow_mut();
                if let Some(cache) = manager.get_page_cache(tab.active_page) {
                    let char_count = cache.chars.len();
                    if char_count > 0 {
                        manager.start_selection(tab.active_page, 0);
                        manager.update_selection(tab.active_page, char_count);
                        manager.end_selection();
                        cx.notify();
                    }
                }
            }
        }
    }

    pub fn clear_text_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.text_selection_manager.borrow_mut().clear_selection();
        }
        cx.notify();
    }

    pub fn open_tab_context_menu(
        &mut self,
        tab_id: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.tab_bar.get_tab_index_by_id(tab_id).is_none() {
            return;
        }

        #[cfg(target_os = "macos")]
        {
            let _ = position;
            let i18n = self.i18n();
            let can_close_others = self.tab_bar.tabs().len() > 1;
            let can_reveal = self
                .tab_bar
                .tabs()
                .iter()
                .any(|tab| tab.id == tab_id && tab.path.is_some());
            self.close_context_menu(cx);
            if let Some(action) = self::macos_context_menu::show_tab_context_menu(
                i18n.close_all_tabs_button,
                i18n.close_other_tabs_button,
                i18n.reveal_in_file_manager_button,
                can_close_others,
                can_reveal,
            ) {
                match action {
                    self::macos_context_menu::MacTabContextMenuAction::CloseAllTabs => {
                        self.close_all_tabs(cx);
                    }
                    self::macos_context_menu::MacTabContextMenuAction::CloseOtherTabs => {
                        self.close_other_tabs(tab_id, cx);
                    }
                    self::macos_context_menu::MacTabContextMenuAction::RevealInFinder => {
                        self.reveal_tab_in_file_manager(tab_id);
                    }
                }
            }
            return;
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.context_menu_open = true;
            self.context_menu_position = Some(position);
            self.context_menu_tab_id = Some(tab_id);
            self.context_menu_note_anchor = None;
            self.context_menu_note_id = None;
            cx.notify();
        }
    }

    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if !self.context_menu_open
            && self.context_menu_position.is_none()
            && self.context_menu_tab_id.is_none()
        {
            return;
        }

        self.context_menu_open = false;
        self.context_menu_position = None;
        self.context_menu_tab_id = None;
        self.context_menu_note_anchor = None;
        self.context_menu_note_id = None;
        cx.notify();
    }

    pub fn has_text_selection(&self) -> bool {
        self.active_tab()
            .and_then(|tab| tab.text_selection_manager.borrow().get_selected_text())
            .is_some()
    }

    pub(super) fn set_text_hover_hit(&mut self, page_index: usize, is_over_text: bool) -> bool {
        let next = if is_over_text {
            self.tab_bar
                .active_tab_id()
                .map(|tab_id| (tab_id, page_index))
        } else {
            None
        };

        if self.text_hover_target != next {
            self.text_hover_target = next;
            true
        } else {
            false
        }
    }

    pub(super) fn text_cursor_style_for_page(&self, page_index: usize) -> gpui::CursorStyle {
        if let Some(note_id) = self.hovered_markdown_note_id()
            && self
                .markdown_note_by_id(note_id)
                .is_some_and(|note| note.page_index == page_index)
        {
            return gpui::CursorStyle::PointingHand;
        }

        let target = self
            .tab_bar
            .active_tab_id()
            .map(|tab_id| (tab_id, page_index));
        if self.text_hover_target == target {
            gpui::CursorStyle::IBeam
        } else {
            gpui::CursorStyle::Arrow
        }
    }

    pub fn close_current_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(active_id) = self.tab_bar.active_tab_id() {
            self.close_tab(active_id, cx);
        }
    }

    // Convenience methods for accessing active tab data in render functions
    pub(super) fn active_tab_display_scroll(
        &self,
    ) -> Option<&gpui_component::VirtualListScrollHandle> {
        self.active_tab().map(|t| &t.display_scroll)
    }

    pub(super) fn active_tab_thumbnail_scroll(
        &self,
    ) -> Option<&gpui_component::VirtualListScrollHandle> {
        self.active_tab().map(|t| &t.thumbnail_scroll)
    }

    pub(super) fn active_tab_pages(&self) -> Option<&Vec<PageSummary>> {
        self.active_tab().map(|t| &t.pages)
    }

    pub(super) fn active_tab_zoom(&self) -> f32 {
        self.active_tab().map(|t| t.zoom).unwrap_or(1.0)
    }

    pub(super) fn active_tab_active_page(&self) -> usize {
        self.active_tab().map(|t| t.active_page).unwrap_or(0)
    }

    #[allow(dead_code)]
    pub(super) fn active_tab_selected_page(&self) -> usize {
        self.active_tab().map(|t| t.selected_page).unwrap_or(0)
    }

    pub(super) fn active_tab_text_selection_manager(
        &self,
    ) -> Option<&std::cell::RefCell<crate::pdf_viewer::text_selection::TextSelectionManager>> {
        self.active_tab().map(|t| &t.text_selection_manager)
    }

    pub(super) fn active_tab_path(&self) -> Option<&PathBuf> {
        self.active_tab().and_then(|t| t.path.as_ref())
    }

    fn show_thumbnail_panel(&self) -> bool {
        self.active_tab_path().is_some()
    }

    fn current_drag_source_tab_id(&self) -> Option<usize> {
        match self.drag_state {
            DragState::Started { source_tab_id } => Some(source_tab_id),
            DragState::Over { source_tab_id, .. } => Some(source_tab_id),
            _ => None,
        }
    }

    fn maybe_start_pending_tab_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((source_tab_id, start_position)) = self.pending_drag_start else {
            return;
        };

        if self.current_drag_source_tab_id().is_some() {
            return;
        }

        let dx = f32::from(position.x) - f32::from(start_position.x);
        let dy = f32::from(position.y) - f32::from(start_position.y);
        let distance_sq = dx * dx + dy * dy;
        let threshold_sq = TAB_DRAG_START_DISTANCE * TAB_DRAG_START_DISTANCE;

        if distance_sq < threshold_sq {
            return;
        }

        self.drag_state = DragState::Started { source_tab_id };
        self.drag_mouse_position = Some(position);
        self.pending_drag_start = None;
        cx.notify();
    }

    fn update_drag_mouse_position(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.maybe_start_pending_tab_drag(position, cx);

        let Some(source_tab_id) = self.current_drag_source_tab_id() else {
            return;
        };

        let mut should_notify = false;
        if self.drag_mouse_position != Some(position) {
            self.drag_mouse_position = Some(position);
            should_notify = true;
        }

        // When cursor leaves tab bar band, clear stale target to avoid "stuck" drag feedback.
        let y: f32 = position.y.into();
        let tab_bar_bottom = TITLE_BAR_HEIGHT + TAB_BAR_HEIGHT;
        if !(TITLE_BAR_HEIGHT..=tab_bar_bottom).contains(&y)
            && !matches!(self.drag_state, DragState::Started { source_tab_id: id } if id == source_tab_id)
        {
            self.drag_state = DragState::Started { source_tab_id };
            should_notify = true;
        }

        if should_notify {
            cx.notify();
        }
    }

    fn finish_tab_drag(&mut self, cx: &mut Context<Self>) {
        if self.pending_drag_start.take().is_some() {
            cx.notify();
        }

        match self.drag_state.clone() {
            DragState::Over {
                source_tab_id,
                target_tab_id,
            } => {
                let source_index = self.tab_bar.get_tab_index_by_id(source_tab_id);
                let target_index = self.tab_bar.get_tab_index_by_id(target_tab_id);
                let mut order_changed = false;
                if let (Some(source_index), Some(target_index)) = (source_index, target_index)
                    && source_index != target_index
                {
                    self.tab_bar.move_tab(source_index, target_index);
                    order_changed = true;
                }
                if order_changed {
                    self.persist_open_tabs();
                }
                self.drag_state = DragState::None;
                self.drag_mouse_position = None;
                cx.notify();
            }
            DragState::Started { .. } => {
                self.drag_state = DragState::None;
                self.drag_mouse_position = None;
                cx.notify();
            }
            DragState::None => {}
        }
    }

    fn render_drag_tab_preview(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let source_tab_id = self.current_drag_source_tab_id()?;
        let position = self.drag_mouse_position?;
        let tab = self
            .tab_bar
            .tabs()
            .iter()
            .find(|tab| tab.id == source_tab_id)?;

        let x: f32 = position.x.into();
        let y: f32 = position.y.into();

        Some(
            div()
                .id("drag-tab-preview")
                .absolute()
                // Keep the pointer outside the preview hit area.
                .left(px(x + 6.0))
                .top(px(y + 6.0))
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().primary.opacity(0.65))
                .bg(cx.theme().secondary.opacity(0.65))
                .shadow_lg()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground.opacity(0.95))
                        .child(tab.file_name()),
                )
                .into_any_element(),
        )
    }

    pub(super) fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.tab_bar.tabs().to_vec();
        let active_tab_id = self.tab_bar.active_tab_id();
        let recent_files_with_positions = self.recent_files_with_positions(&self.recent_files);
        let tab_recent_popup_open = self.recent_popup_open_for(RecentPopupAnchor::TabAddButton);
        let recent_popup_list_scroll = self.recent_popup_list_scroll.clone();
        let i18n = self.i18n();

        // 检查是否有文件打开，如果有，则过滤掉空的 Home 标签
        let has_file_open = tabs.iter().any(|tab| tab.path.is_some());
        let tabs_to_show: Vec<_> = tabs
            .iter()
            .filter(|tab| {
                if has_file_open {
                    // 有文件打开时，只显示有文件的标签
                    tab.path.is_some()
                } else {
                    // 没有文件时，显示所有标签（包括 Home）
                    true
                }
            })
            .collect();

        // 计算拖动指示器位置（基于可见 tab 的索引）
        let insertion_indicator_pos = match &self.drag_state {
            DragState::Over { target_tab_id, .. } => {
                tabs_to_show.iter().position(|tab| tab.id == *target_tab_id)
            }
            _ => None,
        };
        let drag_in_progress = matches!(
            self.drag_state,
            DragState::Started { .. } | DragState::Over { .. }
        );

        div()
            .h(px(TAB_BAR_HEIGHT))
            .w_full()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .flex()
            .items_center()
            .px_3()
            .gap_1()
            .relative() // 使子元素可以使用绝对定位
            .child(
                div().flex_shrink_0().child(
                    Popover::new("new-tab-popover")
                        .anchor(Corner::TopLeft)
                        .appearance(false)
                        .overlay_closable(false)
                        .open(tab_recent_popup_open)
                        .trigger(
                            Button::new("new-tab")
                                .xsmall()
                                .ghost()
                                .icon(
                                    Icon::new(crate::icons::IconName::Plus)
                                        .size_4()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .on_hover({
                                    let viewer = cx.entity();
                                    move |hovered, _, cx| {
                                        let _ = viewer.update(cx, |this, cx| {
                                            this.set_recent_popup_trigger_hovered(
                                                RecentPopupAnchor::TabAddButton,
                                                *hovered,
                                                cx,
                                            );
                                        });
                                    }
                                }),
                        )
                        .content({
                            let viewer = cx.entity();
                            let recent_files_with_positions = recent_files_with_positions.clone();
                            let i18n = i18n;
                            move |_, _window, cx| {
                                Self::render_recent_files_popup_panel(
                                    "new-tab-popup",
                                    1,
                                    i18n,
                                    viewer.clone(),
                                    recent_files_with_positions.clone(),
                                    &recent_popup_list_scroll,
                                    cx,
                                )
                            }
                        }),
                ),
            )
            .child(
                h_flex()
                    .id("tab-scroll")
                    .h_full()
                    .flex_1()
                    .overflow_x_scroll()
                    .track_scroll(&self.tab_bar_scroll)
                    .items_center()
                    .gap_1()
                    .children({
                        let mut elements = Vec::new();

                        for (index, tab) in tabs_to_show.iter().enumerate() {
                            let tab_id = tab.id;
                            let is_active = active_tab_id == Some(tab_id);
                            let show_close_button = is_active || self.hovered_tab_id == Some(tab_id);
                            let close_icon_color = if show_close_button {
                                cx.theme().muted_foreground
                            } else {
                                cx.theme().muted_foreground.opacity(0.0)
                            };
                            let file_name = tab.file_name();
                            let is_home = tab.path.is_none();

                            // 在目标位置前插入垂直线指示器
                            if insertion_indicator_pos == Some(index) {
                                elements.push(
                                    div()
                                        .id(("indicator", index))
                                        .w_px()
                                        .flex_shrink_0()
                                        .h_6()
                                        .rounded_sm()
                                        .bg(cx.theme().primary)
                                        .into_any_element(),
                                );
                            }

                            elements.push(
                                div()
                                    .id(("tab", tab_id))
                                    .h(px(28.))
                                    .px_2()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .rounded_md()
                                    .bg(cx.theme().secondary)
                                    .when(is_active, |this| this.bg(cx.theme().background))
                                    .when(!is_active, |this| {
                                        this.hover(|this| this.bg(cx.theme().secondary.opacity(0.85)))
                                    })
                                    .on_hover({
                                        let viewer = cx.entity();
                                        move |hovered, _, cx| {
                                            let _ = viewer.update(cx, |this, cx| {
                                                if *hovered {
                                                    if this.hovered_tab_id != Some(tab_id) {
                                                        this.hovered_tab_id = Some(tab_id);
                                                        cx.notify();
                                                    }
                                                } else if this.hovered_tab_id == Some(tab_id) {
                                                    this.hovered_tab_id = None;
                                                    if let DragState::Over {
                                                        source_tab_id,
                                                        target_tab_id,
                                                    } = this.drag_state.clone()
                                                        && target_tab_id == tab_id
                                                    {
                                                        this.drag_state = DragState::Started {
                                                            source_tab_id,
                                                        };
                                                    }
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    })
                                    .on_mouse_move(cx.listener(
                                        move |this, event: &MouseMoveEvent, _, cx| {
                                            this.update_drag_mouse_position(event.position, cx);
                                            if let Some(source_tab_id) = this.current_drag_source_tab_id()
                                                && tab_id != source_tab_id
                                                && !matches!(this.drag_state, DragState::Over { source_tab_id: current_source, target_tab_id: current_target } if current_source == source_tab_id && current_target == tab_id)
                                            {
                                                this.drag_state = DragState::Over {
                                                    source_tab_id,
                                                    target_tab_id: tab_id,
                                                };
                                                cx.notify();
                                            }
                                        },
                                    ))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                            if this.tab_bar.get_tab_index_by_id(tab_id).is_some() {
                                                this.pending_drag_start =
                                                    Some((tab_id, event.position));
                                                this.drag_state = DragState::None;
                                                this.drag_mouse_position = None;
                                                cx.notify();
                                            }
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                            this.open_tab_context_menu(tab_id, event.position, cx);
                                        }),
                                    )
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.finish_tab_drag(cx);
                                        }),
                                    )
                                    .when(matches!(self.drag_state, DragState::Started { source_tab_id, .. } if source_tab_id == tab_id)
                                        || matches!(self.drag_state, DragState::Over { source_tab_id, .. } if source_tab_id == tab_id), |div| {
                                        // 如果这个标签正在被拖动，给它特殊的视觉样式
                                        div.border_1()
                                            .border_color(cx.theme().primary)
                                            .bg(cx.theme().selection)
                                            .shadow_lg()
                                    })
                                    .child(
                                        div()
                                            .text_sm()
                                            .whitespace_nowrap()
                                            .text_color(if is_active
                                                || matches!(self.drag_state, DragState::Started { source_tab_id, .. } if source_tab_id == tab_id)
                                                || matches!(self.drag_state, DragState::Over { source_tab_id, .. } if source_tab_id == tab_id) {
                                                cx.theme().foreground
                                            } else {
                                                cx.theme().muted_foreground
                                            })
                                            .child(file_name.clone())
                                            .when(is_home, |this| {
                                                this.text_color(if matches!(self.drag_state, DragState::Started { source_tab_id, .. } if source_tab_id == tab_id)
                                                    || matches!(self.drag_state, DragState::Over { source_tab_id, .. } if source_tab_id == tab_id) {
                                                    cx.theme().foreground
                                                } else {
                                                    cx.theme().muted_foreground.opacity(0.6)
                                                })
                                            }),
                                    )
                                    .child(
                                        Button::new(("close-tab", tab_id))
                                            .xsmall()
                                            .ghost()
                                            .icon(
                                                Icon::new(crate::icons::IconName::WindowClose)
                                                    .size_3()
                                                    .text_color(close_icon_color),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.close_tab(tab_id, cx);
                                            })),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !is_active {
                                            this.switch_to_tab(tab_id, cx);
                                        }
                                    }))
                                    .when(drag_in_progress, |this| this.cursor_grab())
                                    .when(!drag_in_progress, |this| this.cursor_pointer())
                                    .into_any_element(),
                            );
                        }

                        // 处理拖动到末尾的情况
                        if insertion_indicator_pos == Some(tabs_to_show.len()) {
                            elements.push(
                                div()
                                    .id(("indicator", tabs_to_show.len()))
                                    .w_px()
                                    .flex_shrink_0()
                                    .h_6()
                                    .rounded_sm()
                                    .bg(cx.theme().primary)
                                    .into_any_element(),
                            );
                        }

                        elements
                    }),
            )
    }
}
