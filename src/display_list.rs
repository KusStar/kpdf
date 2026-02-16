use super::PdfViewer;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use gpui_component::*;
use std::rc::Rc;

impl PdfViewer {
    pub(super) fn render_display_panel(
        &self,
        page_count: usize,
        display_sizes: Rc<Vec<gpui::Size<Pixels>>>,
        display_panel_width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let i18n = self.i18n();
        div()
            .h_full()
            .flex_1()
            .v_flex()
            .overflow_hidden()
            .bg(cx.theme().muted)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .v_flex()
                    .items_center()
                    .overflow_hidden()
                    .when(page_count == 0, |this| {
                        this.child(
                            div()
                                .h_full()
                                .w(px(display_panel_width))
                                .v_flex()
                                .items_center()
                                .justify_center()
                                .gap_3()
                                .child(
                                    Icon::new(IconName::FolderOpen)
                                        .size_8()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(i18n.no_document_hint()),
                                ),
                        )
                    })
                    .when(page_count > 0, |this| {
                        this.child(
                            div()
                                .h_full()
                                .w(px(display_panel_width))
                                .relative()
                                .child(
                                    v_virtual_list(
                                        cx.entity(),
                                        "display-virtual-list",
                                        display_sizes.clone(),
                                        move |viewer, visible_range, _window, cx| {
                                            let target_width = viewer.display_target_width(_window);
                                            viewer.request_display_load_for_visible_range(
                                                visible_range.clone(),
                                                target_width,
                                                cx,
                                            );

                                            // Note: Text is loaded on-demand when user interacts with the page
                                            // Pdfium is not thread-safe, so we cannot load text asynchronously

                                            visible_range
                                                .map(|ix| {
                                                    let Some(page) = viewer.pages.get(ix) else {
                                                        return div().into_any_element();
                                                    };
                                                    let display_base_width =
                                                        viewer.display_base_width(_window);
                                                    let (page_width, display_height) = viewer
                                                        .display_card_size(
                                                            page,
                                                            display_base_width,
                                                        );

                                                    // Calculate scale factor from PDF points to screen pixels
                                                    let scale = page_width / page.width_pt;

                                                    viewer.render_page_with_text_selection(
                                                        ix,
                                                        page,
                                                        page_width,
                                                        display_height,
                                                        scale,
                                                        cx,
                                                    )
                                                })
                                                .collect::<Vec<_>>()
                                        },
                                    )
                                    .track_scroll(&self.display_scroll)
                                    .into_any_element(),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .right_0()
                                        .bottom_0()
                                        .child(
                                            Scrollbar::vertical(&self.display_scroll)
                                                .scrollbar_show(ScrollbarShow::Always),
                                        ),
                                ),
                        )
                    }),
            )
    }

    fn render_page_with_text_selection(
        &self,
        page_index: usize,
        page: &super::PageSummary,
        page_width: f32,
        page_height: f32,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let i18n = self.i18n();
        let selection_rects =
            self.get_selection_rects_for_page(page_index, page_width, page_height, scale);

        // Get page info for coordinate conversion
        let _page_height_pt = page.height_pt;

        div()
            .id(("display-row", page_index))
            .w_full()
            .h_full()
            .child(
                div()
                    .w(px(page_width))
                    .h(px(page_height))
                    .relative()
                    .overflow_hidden()
                    .bg(cx.theme().background)
                    .when_some(page.display_image.clone(), |this, display_image| {
                        this.child(
                            img(display_image)
                                .size_full()
                                .object_fit(ObjectFit::Contain),
                        )
                    })
                    .when(page.display_image.is_none(), |this| {
                        this.child(
                            div()
                                .size_full()
                                .v_flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .text_color(cx.theme().muted_foreground)
                                .when(page.display_failed, |this| {
                                    this.child(
                                        Icon::new(IconName::File)
                                            .size_8()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(div().text_xs().child(i18n.page_render_failed()))
                                })
                                .when(!page.display_failed, |this| {
                                    this.child(
                                        spinner::Spinner::new()
                                            .large()
                                            .icon(Icon::new(IconName::LoaderCircle))
                                            .color(cx.theme().muted_foreground),
                                    )
                                }),
                        )
                    })
                    // Text interaction overlay (transparent, captures mouse events)
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h_full()
                            .cursor(gpui::CursorStyle::IBeam)
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(
                                    move |this, event: &gpui::MouseDownEvent, window, cx| {
                                        let (local_x, local_y) = this.calculate_page_coordinates(
                                            page_index,
                                            event.position,
                                            page_width,
                                            window,
                                        );

                                        this.handle_text_mouse_down(
                                            page_index,
                                            local_x,
                                            local_y,
                                            page_width,
                                            page_height,
                                            cx,
                                        );
                                    },
                                ),
                            )
                            .on_mouse_move(cx.listener(
                                move |this, event: &gpui::MouseMoveEvent, window, cx| {
                                    let (local_x, local_y) = this.calculate_page_coordinates(
                                        page_index,
                                        event.position,
                                        page_width,
                                        window,
                                    );

                                    this.handle_text_mouse_move(
                                        page_index,
                                        local_x,
                                        local_y,
                                        page_width,
                                        page_height,
                                        cx,
                                    );
                                },
                            ))
                            .on_mouse_up(
                                gpui::MouseButton::Left,
                                cx.listener(move |this, _: &gpui::MouseUpEvent, _window, cx| {
                                    this.handle_text_mouse_up(cx);
                                }),
                            ),
                    )
                    // Render selection highlights (rendered after overlay, so appear on top)
                    .children(
                        selection_rects
                            .into_iter()
                            .map(|(left, top, right, bottom)| {
                                div()
                                    .absolute()
                                    .left(px(left))
                                    .top(px(top))
                                    .w(px(right - left))
                                    .h(px(bottom - top))
                                    .bg(gpui::rgb(0x3390FF)) // Blue color
                                    .opacity(0.3)
                                    .into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_2()
                            .top_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(cx.theme().background.opacity(0.9))
                            .text_xs()
                            .font_medium()
                            .text_color(cx.theme().muted_foreground)
                            .child(i18n.page_badge(page.index + 1)),
                    ),
            )
            .into_any_element()
    }

    fn get_selection_rects_for_page(
        &self,
        page_index: usize,
        page_width_screen: f32,
        page_height_screen: f32,
        scale: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        let manager = self.text_selection_manager.borrow();
        let Some(rects) = manager.get_selection_rects(page_index) else {
            return Vec::new();
        };

        // Get cache page dimensions - same as get_char_bounds_for_page
        let Some(cache) = manager.get_page_cache(page_index) else {
            return Vec::new();
        };

        let page_width_pt = cache.page_width;
        let page_height_pt = cache.page_height;

        // Use the incoming scale parameter which should match the one used in mouse events
        // The incoming scale was calculated as: scale = page_width / page.width_pt (in render_page_with_text_selection)
        let effective_scale = scale;

        // The actual rendered content dimensions should match the incoming page_width_screen and page_height_screen
        // But for ObjectFit::Contain, the actual content may be smaller with centering offsets
        let container_aspect = page_width_screen / page_height_screen;
        let content_aspect = page_width_pt / page_height_pt;

        // Calculate the actual dimensions of the rendered content based on ObjectFit::Contain
        let (final_width, final_height) = if content_aspect > container_aspect {
            // Content is wider relative to its height, so width is limiting factor
            (page_width_screen, page_height_pt * effective_scale)
        } else {
            // Content is taller relative to its width, so height is limiting factor
            (page_width_pt * effective_scale, page_height_screen)
        };

        // Calculate offsets for centering
        let x_offset = (page_width_screen - final_width) / 2.0;
        let y_offset = (page_height_screen - final_height) / 2.0;

        // We'll convert PDF coordinates to screen coordinates considering the actual scaling
        rects
            .into_iter()
            .enumerate()
            .map(|(_idx, (left, top, right, bottom))| {
                // Convert from PDF coordinates to screen coordinates
                // PDF: origin at bottom-left, y increases upward
                // Screen: origin at top-left, y increases downward
                let screen_left = left * effective_scale;
                let screen_right = right * effective_scale;
                let screen_top = (page_height_pt - top) * effective_scale; // Flip Y-axis
                let screen_bottom = (page_height_pt - bottom) * effective_scale; // Flip Y-axis

                // Apply offsets that account for centering due to ObjectFit::Contain
                let final_left = screen_left + x_offset;
                let final_right = screen_right + x_offset;
                let final_top = screen_top + y_offset;
                let final_bottom = screen_bottom + y_offset;

                // Make sure we return (left, top, right, bottom) with proper ordering
                let (ordered_top, ordered_bottom) = if final_top <= final_bottom {
                    (final_top, final_bottom)
                } else {
                    (final_bottom, final_top)
                };

                let (ordered_left, ordered_right) = if final_left <= final_right {
                    (final_left, final_right)
                } else {
                    (final_right, final_left)
                };

                (ordered_left, ordered_top, ordered_right, ordered_bottom)
            })
            .collect()
    }

    /// Get character bounds for debugging visualization - applies the same offset as selection
    fn get_char_bounds_for_page(
        &self,
        page_index: usize,
        page_width_screen: f32,
        page_height_screen: f32,
        scale: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        let manager = self.text_selection_manager.borrow();
        let Some(cache) = manager.get_page_cache(page_index) else {
            return Vec::new();
        };

        let page_width_pt = cache.page_width;
        let page_height_pt = cache.page_height;

        // Use the incoming scale parameter which should match the one used in mouse events
        let effective_scale = scale;

        // Calculate the actual dimensions of the rendered content based on ObjectFit::Contain
        let container_aspect = page_width_screen / page_height_screen;
        let content_aspect = page_width_pt / page_height_pt;

        // Calculate the actual dimensions of the rendered content based on ObjectFit::Contain
        let (final_width, final_height) = if content_aspect > container_aspect {
            // Content is wider relative to its height, so width is limiting factor
            (page_width_screen, page_height_pt * effective_scale)
        } else {
            // Content is taller relative to its width, so height is limiting factor
            (page_width_pt * effective_scale, page_height_screen)
        };

        // Calculate offsets for centering
        let x_offset = (page_width_screen - final_width) / 2.0;
        let y_offset = (page_height_screen - final_height) / 2.0;

        cache
            .chars
            .iter()
            .map(|char_info| {
                // Convert from PDF coordinates to screen coordinates
                // PDF: origin at bottom-left, y increases upward
                // Screen: origin at top-left, y increases downward

                let screen_left = char_info.left * effective_scale;
                let screen_right = char_info.right * effective_scale;
                let screen_top = (page_height_pt - char_info.top) * effective_scale;
                let screen_bottom = (page_height_pt - char_info.bottom) * effective_scale;

                // Apply the same offsets that are used in selection rectangles
                let final_left = screen_left + x_offset;
                let final_right = screen_right + x_offset;
                let final_top = screen_top + y_offset;
                let final_bottom = screen_bottom + y_offset;

                // Ensure coordinates are ordered correctly
                let (ordered_top, ordered_bottom) = if final_top <= final_bottom {
                    (final_top, final_bottom)
                } else {
                    (final_bottom, final_top)
                };

                let (ordered_left, ordered_right) = if final_left <= final_right {
                    (final_left, final_right)
                } else {
                    (final_right, final_left)
                };

                (ordered_left, ordered_top, ordered_right, ordered_bottom)
            })
            .collect()
    }

    /// Calculate local page coordinates from window mouse position
    ///
    /// Returns (local_x, local_y) relative to the page content area
    fn calculate_page_coordinates(
        &self,
        page_index: usize,
        window_pos: Point<Pixels>,
        page_width: f32,
        window: &mut Window,
    ) -> (f32, f32) {
        let scroll_offset = self.display_scroll.offset();

        // Calculate cumulative height of all pages before this one
        let display_base_width = self.display_base_width(window);
        let display_sizes = self.display_item_sizes(display_base_width);
        let cumulative_height: f32 = display_sizes
            .iter()
            .take(page_index)
            .map(|s| f32::from(s.height))
            .sum();

        // Calculate horizontal centering offset (pages are centered in the panel)
        let display_panel_width = self.display_panel_width(window);
        let horizontal_offset = (display_panel_width - page_width) / 2.0;

        // Constants for layout offsets
        let content_offset_y = 35.0; // Title bar height
        let sidebar_width = super::SIDEBAR_WIDTH;

        // Convert window coordinates to local page coordinates
        let local_x = f32::from(window_pos.x) - sidebar_width - horizontal_offset;
        let local_y = f32::from(window_pos.y)
            - content_offset_y
            - cumulative_height
            - f32::from(scroll_offset.y);

        (local_x, local_y)
    }

    fn handle_text_mouse_down(
        &mut self,
        page_index: usize,
        local_x: f32,
        local_y: f32,
        page_width_screen: f32,
        page_height_screen: f32,
        cx: &mut Context<Self>,
    ) {
        // Ensure text is loaded before handling mouse down
        self.ensure_page_text_loaded(page_index);

        let manager = self.text_selection_manager.borrow();

        // Get cache page dimensions - these must match the dimensions used in get_char_bounds_for_page
        let Some(cache) = manager.get_page_cache(page_index) else {
            self.text_selection_manager.borrow_mut().clear_selection();
            cx.notify();
            return;
        };

        let page_width_pt = cache.page_width;
        let _page_height_pt = cache.page_height;

        // Calculate scale factor (same as in render_page_with_text_selection)
        let scale = page_width_screen / page_width_pt;

        // Use the improved method to find character at screen position
        let char_index = cache.find_char_at_screen_position(
            local_x,
            local_y,
            (0.0, 0.0, page_width_screen, page_height_screen), // Relative to page container
            scale,
        );

        drop(manager); // Release borrow before mutable borrow

        if let Some(char_index) = char_index {
            self.text_selection_manager
                .borrow_mut()
                .start_selection(page_index, char_index);
        } else {
            self.text_selection_manager.borrow_mut().clear_selection();
        }
        cx.notify();
    }

    fn handle_text_mouse_move(
        &mut self,
        page_index: usize,
        local_x: f32,
        local_y: f32,
        page_width_screen: f32,
        page_height_screen: f32,
        cx: &mut Context<Self>,
    ) {
        // Ensure text is loaded before handling mouse move
        self.ensure_page_text_loaded(page_index);

        let manager = self.text_selection_manager.borrow();

        if !manager.is_selecting() {
            return;
        }

        // Get cache page dimensions - these must match the dimensions used in get_char_bounds_for_page
        let Some(cache) = manager.get_page_cache(page_index) else {
            return;
        };

        let page_width_pt = cache.page_width;
        let _page_height_pt = cache.page_height;

        // Use the same scale as passed to rendering functions
        let scale = page_width_screen / page_width_pt;

        // Use the improved method to find character at screen position
        let char_index = cache.find_char_at_screen_position(
            local_x,
            local_y,
            (0.0, 0.0, page_width_screen, page_height_screen), // Relative to page container
            scale,
        );

        drop(manager);

        if let Some(char_index) = char_index {
            self.text_selection_manager
                .borrow_mut()
                .update_selection(page_index, char_index);
            cx.notify();
        }
    }

    fn handle_text_mouse_up(&mut self, cx: &mut Context<Self>) {
        self.text_selection_manager.borrow_mut().end_selection();
        cx.notify();
    }

    fn load_page_text(
        &self,
        page_index: usize,
        _path: std::path::PathBuf,
        _cx: &mut Context<Self>,
    ) {
        // Check if we already have text loaded for this page
        if self
            .text_selection_manager
            .borrow()
            .get_page_cache(page_index)
            .is_some()
        {
            return;
        }

        // Mark as loading to prevent duplicate requests
        // Note: Pdfium is not thread-safe, so we must load text synchronously
        // We'll load on demand when user interacts with the page
    }

    fn ensure_page_text_loaded(&self, page_index: usize) {
        // Check if already loaded
        if self
            .text_selection_manager
            .borrow()
            .get_page_cache(page_index)
            .is_some()
        {
            return;
        }

        // Load text synchronously (pdfium is not thread-safe)
        if let Some(path) = &self.path {
            match crate::pdf_viewer::utils::load_page_text_for_selection(path, page_index) {
                Ok(Some((page_index, page_width, page_height, chars))) => {
                    let _char_count = chars.len();
                    if let Ok(mut manager) = self.text_selection_manager.try_borrow_mut() {
                        manager.load_cached_text(page_index, page_width, page_height, chars);
                    }
                }
                Ok(None) => {
                    // No text found on page
                }
                Err(_e) => {
                    // Failed to load text for page
                }
            }
        }
    }
}
