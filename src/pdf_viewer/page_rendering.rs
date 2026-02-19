impl PdfViewer {
    fn select_page(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            if index < tab.pages.len() {
                tab.selected_page = index;
                tab.active_page = index;
                self.sync_scroll_to_selected();
                self.persist_current_file_position();
                cx.notify();
            }
        }
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            if tab.active_page > 0 {
                let new_page = tab.active_page - 1;
                self.select_page(new_page, cx);
            }
        }
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            if tab.active_page + 1 < tab.pages.len() {
                let new_page = tab.active_page + 1;
                self.select_page(new_page, cx);
            }
        }
    }

    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.zoom = (tab.zoom + ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
            cx.notify();
        }
    }

    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.zoom = (tab.zoom - ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
            cx.notify();
        }
    }

    fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.zoom = 1.0;
            cx.notify();
        }
    }

    fn sync_scroll_to_selected(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.suppress_display_scroll_sync_once = true;
            tab.thumbnail_scroll
                .scroll_to_item(tab.selected_page, ScrollStrategy::Center);
            tab.display_scroll
                .scroll_to_item(tab.selected_page, ScrollStrategy::Top);
        }
    }

    fn schedule_thumbnail_sync_after_display_scroll(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };

        tab.display_scroll_sync_epoch = tab.display_scroll_sync_epoch.wrapping_add(1);
        let sync_epoch = tab.display_scroll_sync_epoch;
        let tab_id = tab.id;

        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(DISPLAY_SCROLL_SYNC_DELAY_MS))
                .await;

            let _ = view.update(cx, |this, cx| {
                let path_to_save = {
                    let Some(tab) = this.tab_bar.get_active_tab_mut() else {
                        return;
                    };
                    if tab.id != tab_id
                        || tab.display_scroll_sync_epoch != sync_epoch
                        || tab.pages.is_empty()
                    {
                        return;
                    }

                    let next_active = tab
                        .last_display_visible_range
                        .as_ref()
                        .map(|range| range.start.min(tab.pages.len().saturating_sub(1)))
                        .unwrap_or_else(|| tab.active_page.min(tab.pages.len().saturating_sub(1)));

                    let page_index_changed = tab.active_page != next_active;

                    if page_index_changed {
                        tab.active_page = next_active;
                        // Save position directly
                        if let Some(ref path) = tab.path {
                            if !tab.pages.is_empty() {
                                let page_index =
                                    tab.active_page.min(tab.pages.len().saturating_sub(1));
                                tab.last_saved_position = Some((path.clone(), page_index));
                                Some((path.clone(), page_index))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                // Save file position outside the mutable borrow
                if let Some((path, page_index)) = path_to_save {
                    this.save_file_position(&path, page_index);
                }

                // Get tab again for scroll operation
                if let Some(tab) = this.tab_bar.get_active_tab_mut() {
                    let next_active = tab.active_page;
                    tab.thumbnail_scroll
                        .scroll_to_item(next_active, ScrollStrategy::Center);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn on_display_scroll_offset_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            let offset = tab.display_scroll.offset();
            let has_changed = tab
                .last_display_scroll_offset
                .map(|last| last != offset)
                .unwrap_or(false);
            tab.last_display_scroll_offset = Some(offset);

            if has_changed && !tab.pages.is_empty() {
                if tab.suppress_display_scroll_sync_once {
                    tab.suppress_display_scroll_sync_once = false;
                    return;
                }
                self.schedule_thumbnail_sync_after_display_scroll(cx);
            }
        }
    }

    fn schedule_restore_current_page_after_layout_change(
        &mut self,
        keep_page: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self.tab_bar.active_tab_id() else {
            return;
        };

        self.resize_restore_epoch = self.resize_restore_epoch.wrapping_add(1);
        let restore_epoch = self.resize_restore_epoch;

        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;

            let _ = view.update(cx, |this, cx| {
                if this.resize_restore_epoch != restore_epoch {
                    return;
                }

                let Some(tab) = this.tab_bar.get_active_tab_mut() else {
                    return;
                };
                if tab.id != tab_id || tab.pages.is_empty() {
                    return;
                }

                let page_index = keep_page.min(tab.pages.len().saturating_sub(1));
                tab.active_page = page_index;
                tab.selected_page = page_index;
                tab.last_display_visible_range =
                    Some(page_index..page_index.saturating_add(1).min(tab.pages.len()));
                tab.suppress_display_scroll_sync_once = true;
                tab.display_scroll
                    .scroll_to_item(page_index, ScrollStrategy::Top);
                tab.thumbnail_scroll
                    .scroll_to_item(page_index, ScrollStrategy::Center);
                tab.last_display_scroll_offset = Some(tab.display_scroll.offset());
                cx.notify();
            });
        })
        .detach();
    }

    fn thumbnail_base_width(&self) -> f32 {
        (SIDEBAR_WIDTH - THUMB_HORIZONTAL_PADDING).max(THUMB_MIN_WIDTH)
    }

    fn thumbnail_card_size(&self, page: &PageSummary) -> (f32, f32) {
        let width = self.thumbnail_base_width();
        let aspect_ratio = if page.width_pt > 1.0 {
            page.height_pt / page.width_pt
        } else {
            1.4
        };
        let height = width * aspect_ratio;
        (width, height)
    }

    fn thumbnail_row_height(&self, page: &PageSummary) -> f32 {
        let (_, height) = self.thumbnail_card_size(page);
        height + THUMB_VERTICAL_PADDING
    }

    fn thumbnail_item_sizes(&self, pages: &[PageSummary]) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            pages
                .iter()
                .map(|page| size(px(0.), px(self.thumbnail_row_height(page))))
                .collect(),
        )
    }

    fn thumbnail_target_width(&self, window: &Window) -> u32 {
        let width = self.thumbnail_base_width() * window.scale_factor();
        width.clamp(1.0, i32::MAX as f32).round() as u32
    }

    fn request_thumbnail_load_from_candidates(
        &mut self,
        candidate_order: Vec<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        let language = self.language;
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if candidate_order.is_empty() || tab.pages.is_empty() {
            return;
        }

        let Some(path) = tab.path.clone() else {
            return;
        };

        if tab.thumbnail_inflight_tasks >= THUMB_MAX_PARALLEL_TASKS {
            return;
        }

        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for ix in candidate_order {
            if !seen.insert(ix) {
                continue;
            }

            let Some(page) = tab.pages.get(ix) else {
                continue;
            };

            let needs_render =
                page.thumbnail_image.is_none() || page.thumbnail_render_width < target_width;
            if needs_render && !page.thumbnail_failed {
                pending.push(ix);
                if pending.len() >= THUMB_BATCH_SIZE {
                    break;
                }
            }
        }

        if pending.is_empty() {
            return;
        }

        for ix in &pending {
            tab.thumbnail_loading.insert(*ix);
        }
        tab.thumbnail_inflight_tasks = tab.thumbnail_inflight_tasks.saturating_add(1);
        let epoch = tab.thumbnail_epoch;
        let tab_id = tab.id;

        cx.spawn(async move |view, cx| {
            let load_result = cx
                .background_executor()
                .spawn(async move {
                    let loaded = load_display_images(&path, &pending, target_width, language);
                    (pending, target_width, loaded)
                })
                .await;

            let _ = view.update(cx, |this, cx| {
                let Some(tab) = this.tab_bar.get_active_tab_mut() else {
                    return;
                };
                if tab.id != tab_id || tab.thumbnail_epoch != epoch {
                    return;
                }

                tab.thumbnail_inflight_tasks = tab.thumbnail_inflight_tasks.saturating_sub(1);

                let (requested_indices, loaded_target_width, loaded_result) = load_result;
                let mut loaded_indices = HashSet::new();

                match loaded_result {
                    Ok(images) => {
                        for (ix, image) in images {
                            if let Some(page) = tab.pages.get_mut(ix) {
                                page.thumbnail_image = Some(image);
                                page.thumbnail_render_width = loaded_target_width;
                                page.thumbnail_failed = false;
                                loaded_indices.insert(ix);
                            }
                        }
                    }
                    Err(_) => {}
                }

                for ix in requested_indices {
                    tab.thumbnail_loading.remove(&ix);
                    if !loaded_indices.contains(&ix)
                        && let Some(page) = tab.pages.get_mut(ix)
                    {
                        page.thumbnail_failed = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_thumbnail_load_for_visible_range(
        &mut self,
        visible_range: std::ops::Range<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if visible_range.is_empty() || tab.pages.is_empty() {
            return;
        }

        if tab.thumbnail_inflight_tasks == 0 && !tab.thumbnail_loading.is_empty() {
            tab.thumbnail_loading.clear();
        }

        tab.last_thumbnail_visible_range = Some(visible_range.clone());

        let mut candidate_order = Vec::with_capacity(visible_range.len());
        candidate_order.extend(visible_range.clone());
        self.request_thumbnail_load_from_candidates(candidate_order, target_width, cx);
    }

    fn display_available_width(&self, window: &Window) -> f32 {
        let viewport_width: f32 = window.viewport_size().width.into();
        let sidebar_width = if self.show_thumbnail_panel() {
            SIDEBAR_WIDTH
        } else {
            0.0
        };
        (viewport_width - sidebar_width).max(DISPLAY_MIN_WIDTH)
    }

    fn display_panel_width(&self, window: &Window, zoom: f32) -> f32 {
        let available_width = self.display_available_width(window);
        (available_width * zoom).clamp(DISPLAY_MIN_WIDTH, available_width)
    }

    fn display_base_width(&self, window: &Window, zoom: f32) -> f32 {
        self.display_panel_width(window, zoom)
    }

    fn display_card_size(&self, page: &PageSummary, base_width: f32) -> (f32, f32) {
        let width = base_width.max(DISPLAY_MIN_WIDTH);
        let aspect_ratio = if page.width_pt > 1.0 {
            page.height_pt / page.width_pt
        } else {
            1.4
        };
        let height = width * aspect_ratio;
        (width, height)
    }

    fn display_row_height(&self, page: &PageSummary, base_width: f32) -> f32 {
        let (_, height) = self.display_card_size(page, base_width);
        height
    }

    fn display_item_sizes(
        &self,
        pages: &[PageSummary],
        base_width: f32,
    ) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            pages
                .iter()
                .map(|page| size(px(0.), px(self.display_row_height(page, base_width))))
                .collect(),
        )
    }

    fn display_target_width(&self, window: &Window, zoom: f32) -> u32 {
        let width = self.display_panel_width(window, zoom) * window.scale_factor();
        width.clamp(1.0, i32::MAX as f32).round() as u32
    }

    fn request_display_load_from_candidates(
        &mut self,
        candidate_order: Vec<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        let language = self.language;
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if candidate_order.is_empty() || tab.pages.is_empty() {
            return;
        }

        let Some(path) = tab.path.clone() else {
            return;
        };

        if tab.display_inflight_tasks >= DISPLAY_MAX_PARALLEL_TASKS {
            return;
        }

        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for ix in candidate_order {
            if !seen.insert(ix) {
                continue;
            }

            let Some(page) = tab.pages.get(ix) else {
                continue;
            };

            let needs_render =
                page.display_image.is_none() || page.display_render_width < target_width;
            if needs_render && !page.display_failed {
                pending.push(ix);
                if pending.len() >= DISPLAY_BATCH_SIZE {
                    break;
                }
            }
        }

        if pending.is_empty() {
            return;
        }

        for ix in &pending {
            tab.display_loading.insert(*ix);
        }
        tab.display_inflight_tasks = tab.display_inflight_tasks.saturating_add(1);
        let epoch = tab.display_epoch;
        let tab_id = tab.id;

        cx.spawn(async move |view, cx| {
            let load_result = cx
                .background_executor()
                .spawn(async move {
                    let loaded = load_display_images(&path, &pending, target_width, language);
                    (pending, target_width, loaded)
                })
                .await;

            let _ = view.update(cx, |this, cx| {
                let Some(tab) = this.tab_bar.get_active_tab_mut() else {
                    return;
                };
                if tab.id != tab_id || tab.display_epoch != epoch {
                    return;
                }

                tab.display_inflight_tasks = tab.display_inflight_tasks.saturating_sub(1);

                let (requested_indices, loaded_target_width, loaded_result) = load_result;
                let mut loaded_indices = HashSet::new();

                match loaded_result {
                    Ok(images) => {
                        for (ix, image) in images {
                            if let Some(page) = tab.pages.get_mut(ix) {
                                page.display_image = Some(image);
                                page.display_render_width = loaded_target_width;
                                page.display_failed = false;
                                loaded_indices.insert(ix);
                            }
                        }
                    }
                    Err(_) => {}
                }

                for ix in requested_indices {
                    tab.display_loading.remove(&ix);
                    if !loaded_indices.contains(&ix)
                        && let Some(page) = tab.pages.get_mut(ix)
                    {
                        page.display_failed = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_display_load_for_visible_range(
        &mut self,
        visible_range: std::ops::Range<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if visible_range.is_empty() || tab.pages.is_empty() {
            return;
        }

        if tab.display_inflight_tasks == 0 && !tab.display_loading.is_empty() {
            tab.display_loading.clear();
        }

        tab.last_display_visible_range = Some(visible_range.clone());

        let mut candidate_order = Vec::with_capacity(visible_range.len());
        candidate_order.extend(visible_range.clone());

        self.request_display_load_from_candidates(candidate_order, target_width, cx);
    }
}
