use super::PdfViewer;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use gpui_component::text::TextView;
use gpui_component::*;
use std::rc::Rc;

const MARKDOWN_NOTE_BUBBLE_OFFSET_X: f32 = 14.0;
const MARKDOWN_NOTE_BUBBLE_PADDING: f32 = 8.0;

#[derive(Clone)]
struct MarkdownNoteMarker {
    id: u64,
    x: f32,
    y: f32,
    preview: String,
    bubble_left: f32,
    bubble_top: f32,
}

impl PdfViewer {
    pub(super) fn render_display_panel(
        &self,
        page_count: usize,
        display_sizes: Rc<Vec<gpui::Size<Pixels>>>,
        display_panel_width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let i18n = self.i18n();
        let zoom = self.active_tab_zoom();
        let is_home_tab = self.active_tab_path().is_none();
        let recent_files_with_positions = self.recent_files_with_positions(&self.recent_files);

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
                    .when(page_count == 0 && is_home_tab, |this| {
                        this.child(
                            div()
                                .h_full()
                                .w(px(display_panel_width))
                                .v_flex()
                                .items_center()
                                .justify_center()
                                .p_4()
                                .child(
                                    div()
                                        .w(px(display_panel_width.min(560.0)))
                                        .max_w_full()
                                        .v_flex()
                                        .gap_3()
                                        .popover_style(cx)
                                        .p_4()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_medium()
                                                .text_color(cx.theme().foreground)
                                                .child(i18n.file_not_opened),
                                        )
                                        .child(Self::render_recent_files_list_content(
                                            2,
                                            i18n,
                                            cx.entity(),
                                            recent_files_with_positions.clone(),
                                            &self.recent_home_list_scroll,
                                            true,
                                            cx,
                                        )),
                                ),
                        )
                    })
                    .when(page_count == 0 && !is_home_tab, |this| {
                        this.child(
                            div()
                                .h_full()
                                .w(px(display_panel_width))
                                .v_flex()
                                .items_center()
                                .justify_center()
                                .gap_3()
                                .child(
                                    Icon::new(crate::icons::IconName::File)
                                        .size_8()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(i18n.no_pages),
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
                                        move |viewer, visible_range, window, cx| {
                                            let target_width =
                                                viewer.display_target_width(window, zoom);
                                            viewer.request_display_load_for_visible_range(
                                                visible_range.clone(),
                                                target_width,
                                                cx,
                                            );

                                            // Note: Text is loaded on-demand when user interacts with the page
                                            // Pdfium is not thread-safe, so we cannot load text asynchronously

                                            visible_range
                                                .map(|ix| {
                                                    let Some(pages) = viewer.active_tab_pages()
                                                    else {
                                                        return div().into_any_element();
                                                    };
                                                    let Some(page) = pages.get(ix) else {
                                                        return div().into_any_element();
                                                    };
                                                    let display_base_width =
                                                        viewer.display_base_width(window, zoom);
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
                                                        window,
                                                        cx,
                                                    )
                                                })
                                                .collect::<Vec<_>>()
                                        },
                                    )
                                    .track_scroll(self.active_tab_display_scroll().unwrap())
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
                                            Scrollbar::vertical(
                                                self.active_tab_display_scroll().unwrap(),
                                            )
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let i18n = self.i18n();
        let selection_rects =
            self.get_selection_rects_for_page(page_index, page_width, page_height, scale);
        let markdown_note_markers =
            self.markdown_note_markers_for_page(page_index, page_width, page_height);

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
                                        Icon::new(crate::icons::IconName::File)
                                            .size_8()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(div().text_xs().child(i18n.page_render_failed))
                                })
                                .when(!page.display_failed, |this| {
                                    this.child(
                                        spinner::Spinner::new()
                                            .large()
                                            .icon(Icon::new(crate::icons::IconName::LoaderCircle))
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
                            .cursor(self.text_cursor_style_for_page(page_index))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(
                                    move |this, event: &gpui::MouseDownEvent, window, cx| {
                                        // Close context menu if open
                                        if this.context_menu_open {
                                            this.close_context_menu(cx);
                                            return;
                                        }

                                        let (local_x, local_y) = this.calculate_page_coordinates(
                                            page_index,
                                            event.position,
                                            page_width,
                                            window,
                                        );

                                        if let Some(note_id) = this
                                            .hit_test_markdown_note_id_on_page(
                                                page_index,
                                                local_x,
                                                local_y,
                                                page_width,
                                                page_height,
                                            )
                                        {
                                            let _ = this.set_markdown_note_hover_id(Some(note_id));
                                            this.open_markdown_note_editor_for_edit(
                                                note_id, window, cx,
                                            );
                                            return;
                                        }

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
                            .on_mouse_down(
                                gpui::MouseButton::Right,
                                cx.listener(
                                    move |this, event: &gpui::MouseDownEvent, window, cx| {
                                        let (local_x, local_y) = this.calculate_page_coordinates(
                                            page_index,
                                            event.position,
                                            page_width,
                                            window,
                                        );
                                        let note_id = this.hit_test_markdown_note_id_on_page(
                                            page_index,
                                            local_x,
                                            local_y,
                                            page_width,
                                            page_height,
                                        );
                                        let note_anchor = if note_id.is_some() {
                                            None
                                        } else {
                                            this.page_local_screen_to_note_anchor(
                                                page_index,
                                                local_x,
                                                local_y,
                                                page_width,
                                                page_height,
                                            )
                                        };
                                        if note_id.is_some()
                                            || note_anchor.is_some()
                                            || this.has_text_selection()
                                        {
                                            this.open_page_context_menu(
                                                event.position,
                                                note_anchor,
                                                note_id,
                                                cx,
                                            );
                                        }
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
                    .children(markdown_note_markers.iter().map(|marker| {
                        let is_hovered = self.hovered_markdown_note_id() == Some(marker.id);
                        let bubble_bg = if is_hovered {
                            cx.theme().secondary.opacity(0.96)
                        } else {
                            cx.theme().secondary.opacity(0.88)
                        };
                        let bubble_text = if is_hovered {
                            cx.theme().foreground
                        } else {
                            cx.theme().foreground.opacity(0.92)
                        };
                        let note_id = marker.id;
                        div()
                            .absolute()
                            .left(px(marker.bubble_left))
                            .top(px(marker.bubble_top))
                            .rounded_lg()
                            .border_1()
                            .border_color(gpui::rgb(0x000000))
                            .bg(bubble_bg)
                            .shadow_md()
                            .overflow_hidden()
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .on_mouse_move(cx.listener(
                                move |this, _: &gpui::MouseMoveEvent, _, cx| {
                                    if this.set_markdown_note_hover_id(Some(note_id)) {
                                        cx.notify();
                                    }
                                },
                            ))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |this, _: &gpui::MouseDownEvent, window, cx| {
                                    let _ = this.set_markdown_note_hover_id(Some(note_id));
                                    this.open_markdown_note_editor_for_edit(note_id, window, cx);
                                    cx.stop_propagation();
                                }),
                            )
                            .on_mouse_down(
                                gpui::MouseButton::Right,
                                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                    let _ = this.set_markdown_note_hover_id(Some(note_id));
                                    this.open_page_context_menu(
                                        event.position,
                                        None,
                                        Some(note_id),
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }),
                            )
                            .child({
                                let preview_id: SharedString =
                                    format!("markdown-note-bubble-{}", marker.id).into();
                                div().text_color(bubble_text).child(
                                    TextView::markdown(
                                        preview_id,
                                        marker.preview.clone(),
                                        window,
                                        cx,
                                    )
                                    .scrollable(false)
                                    .selectable(false),
                                )
                            })
                            .into_any_element()
                    })),
            )
            .into_any_element()
    }

    fn page_content_transform(
        page_width_pt: f32,
        page_height_pt: f32,
        page_width_screen: f32,
        page_height_screen: f32,
        scale: f32,
    ) -> Option<(f32, f32, f32, f32)> {
        if page_width_pt <= 0.0
            || page_height_pt <= 0.0
            || page_width_screen <= 0.0
            || page_height_screen <= 0.0
            || scale <= 0.0
        {
            return None;
        }

        let container_aspect = page_width_screen / page_height_screen;
        let content_aspect = page_width_pt / page_height_pt;
        let (content_width, content_height) = if content_aspect > container_aspect {
            (page_width_screen, page_height_pt * scale)
        } else {
            (page_width_pt * scale, page_height_screen)
        };
        let x_offset = (page_width_screen - content_width) / 2.0;
        let y_offset = (page_height_screen - content_height) / 2.0;
        Some((content_width, content_height, x_offset, y_offset))
    }

    fn page_local_screen_to_note_anchor(
        &self,
        page_index: usize,
        local_x: f32,
        local_y: f32,
        page_width_screen: f32,
        page_height_screen: f32,
    ) -> Option<super::MarkdownNoteAnchor> {
        let page = self.active_tab_pages()?.get(page_index)?;
        if page.width_pt <= 0.0 || page.height_pt <= 0.0 {
            return None;
        }
        let scale = page_width_screen / page.width_pt;
        let (content_width, content_height, x_offset, y_offset) = Self::page_content_transform(
            page.width_pt,
            page.height_pt,
            page_width_screen,
            page_height_screen,
            scale,
        )?;
        let content_x = local_x - x_offset;
        let content_y = local_y - y_offset;
        if content_x < 0.0
            || content_x > content_width
            || content_y < 0.0
            || content_y > content_height
        {
            return None;
        }

        let pdf_x = content_x / scale;
        let pdf_y = (content_height - content_y) / scale;
        Some(super::MarkdownNoteAnchor {
            page_index,
            x_ratio: (pdf_x / page.width_pt).clamp(0.0, 1.0),
            y_ratio: (pdf_y / page.height_pt).clamp(0.0, 1.0),
        })
    }

    fn note_anchor_to_page_local_screen(
        &self,
        page_index: usize,
        anchor: &super::MarkdownNoteAnchor,
        page_width_screen: f32,
        page_height_screen: f32,
    ) -> Option<(f32, f32)> {
        let page = self.active_tab_pages()?.get(page_index)?;
        if page.width_pt <= 0.0 || page.height_pt <= 0.0 {
            return None;
        }
        let scale = page_width_screen / page.width_pt;
        let (_content_width, content_height, x_offset, y_offset) = Self::page_content_transform(
            page.width_pt,
            page.height_pt,
            page_width_screen,
            page_height_screen,
            scale,
        )?;
        let pdf_x = anchor.x_ratio.clamp(0.0, 1.0) * page.width_pt;
        let pdf_y = anchor.y_ratio.clamp(0.0, 1.0) * page.height_pt;
        let local_x = x_offset + pdf_x * scale;
        let local_y = y_offset + (content_height - pdf_y * scale);
        Some((local_x, local_y))
    }

    fn markdown_note_markers_for_page(
        &self,
        page_index: usize,
        page_width_screen: f32,
        page_height_screen: f32,
    ) -> Vec<MarkdownNoteMarker> {
        self.active_tab_markdown_notes_for_page(page_index)
            .into_iter()
            .filter_map(|note| {
                let anchor = super::MarkdownNoteAnchor {
                    page_index,
                    x_ratio: note.x_ratio,
                    y_ratio: note.y_ratio,
                };
                self.note_anchor_to_page_local_screen(
                    page_index,
                    &anchor,
                    page_width_screen,
                    page_height_screen,
                )
                .map(|(x, y)| {
                    let preview = Self::note_bubble_preview_text(&note.markdown);
                    let (bubble_left, bubble_top) =
                        Self::note_bubble_position(x, y, page_width_screen, page_height_screen);
                    MarkdownNoteMarker {
                        id: note.id,
                        x,
                        y,
                        preview,
                        bubble_left,
                        bubble_top,
                    }
                })
            })
            .collect::<Vec<_>>()
    }

    fn note_bubble_preview_text(markdown: &str) -> String {
        markdown.to_string()
    }

    fn note_bubble_position(
        marker_x: f32,
        marker_y: f32,
        page_width_screen: f32,
        page_height_screen: f32,
    ) -> (f32, f32) {
        let left_min = MARKDOWN_NOTE_BUBBLE_PADDING;
        let left_max = (page_width_screen - MARKDOWN_NOTE_BUBBLE_PADDING).max(left_min);
        let left = (marker_x + MARKDOWN_NOTE_BUBBLE_OFFSET_X).clamp(left_min, left_max);

        let top_min = MARKDOWN_NOTE_BUBBLE_PADDING;
        let top_max = (page_height_screen - MARKDOWN_NOTE_BUBBLE_PADDING).max(top_min);
        let top = marker_y.clamp(top_min, top_max);

        (left, top)
    }

    fn hit_test_markdown_note_id_on_page(
        &self,
        page_index: usize,
        local_x: f32,
        local_y: f32,
        page_width_screen: f32,
        page_height_screen: f32,
    ) -> Option<u64> {
        let hit_radius = super::MARKDOWN_NOTE_MARKER_RADIUS + 6.0;
        let markers =
            self.markdown_note_markers_for_page(page_index, page_width_screen, page_height_screen);
        markers.into_iter().find_map(|marker| {
            let dx = marker.x - local_x;
            let dy = marker.y - local_y;
            ((dx * dx + dy * dy) <= hit_radius * hit_radius).then_some(marker.id)
        })
    }

    fn get_selection_rects_for_page(
        &self,
        page_index: usize,
        page_width_screen: f32,
        page_height_screen: f32,
        scale: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        let Some(manager_ref) = self.active_tab_text_selection_manager() else {
            return Vec::new();
        };
        let manager = manager_ref.borrow();
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

    /// Calculate local page coordinates from window mouse position
    ///
    /// Returns (local_x, local_y) relative to the page container (including margins due to ObjectFit::Contain).
    /// The caller (e.g., find_char_at_screen_position) is responsible for handling content centering offsets.
    fn calculate_page_coordinates(
        &self,
        page_index: usize,
        window_pos: Point<Pixels>,
        page_width: f32,
        window: &mut Window,
    ) -> (f32, f32) {
        let Some(scroll_handle) = self.active_tab_display_scroll() else {
            return (0.0, 0.0);
        };
        let scroll_offset = scroll_handle.offset();
        let zoom = self.active_tab_zoom();

        // Calculate cumulative height of all pages before this one
        let display_base_width = self.display_base_width(window, zoom);
        let Some(pages) = self.active_tab_pages() else {
            return (0.0, 0.0);
        };
        let display_sizes = self.display_item_sizes(pages, display_base_width);
        let cumulative_height: f32 = display_sizes
            .iter()
            .take(page_index)
            .map(|s| f32::from(s.height))
            .sum();

        // Calculate horizontal centering offset (pages are centered in the panel)
        let display_panel_width = self.display_panel_width(window, zoom);
        let horizontal_offset = (display_panel_width - page_width) / 2.0;

        // Keep this aligned with the actual top-bar layout in `pdf_viewer/mod.rs`.
        let content_offset_y = super::TITLE_BAR_HEIGHT + super::TAB_BAR_HEIGHT;
        let sidebar_width = if self.show_thumbnail_panel() {
            super::SIDEBAR_WIDTH
        } else {
            0.0
        };

        // Convert window coordinates to local page container coordinates
        // Note: Do NOT subtract content centering offset here.
        // find_char_at_screen_position handles ObjectFit::Contain centering internally.
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
        let _ = self.set_markdown_note_hover_id(None);

        // Ensure text is loaded before handling mouse down
        self.ensure_page_text_loaded(page_index);

        let Some(manager_ref) = self.active_tab_text_selection_manager() else {
            return;
        };
        let manager = manager_ref.borrow();

        // Get cache page dimensions - these must match the dimensions used in get_char_bounds_for_page
        let Some(cache) = manager.get_page_cache(page_index) else {
            drop(manager);
            let _ = self.set_text_hover_hit(page_index, false);
            if let Some(manager_ref) = self.active_tab_text_selection_manager() {
                manager_ref.borrow_mut().clear_selection();
            }
            cx.notify();
            return;
        };

        let page_width_pt = cache.page_width;
        let _page_height_pt = cache.page_height;

        // Calculate scale factor (same as in render_page_with_text_selection)
        let scale = page_width_screen / page_width_pt;

        // Only allow entering text-edit/selection state when the pointer is on text.
        let char_index = cache.hit_test_char_at_screen_position(
            local_x,
            local_y,
            (0.0, 0.0, page_width_screen, page_height_screen), // Relative to page container
            scale,
        );

        drop(manager); // Release borrow before mutable borrow
        let _ = self.set_text_hover_hit(page_index, char_index.is_some());

        if let Some(char_index) = char_index {
            if let Some(manager_ref) = self.active_tab_text_selection_manager() {
                manager_ref
                    .borrow_mut()
                    .start_selection(page_index, char_index);
            }
        } else {
            if let Some(manager_ref) = self.active_tab_text_selection_manager() {
                manager_ref.borrow_mut().clear_selection();
            }
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

        let hovered_note_id = self.hit_test_markdown_note_id_on_page(
            page_index,
            local_x,
            local_y,
            page_width_screen,
            page_height_screen,
        );
        let note_hover_changed = self.set_markdown_note_hover_id(hovered_note_id);

        let Some(manager_ref) = self.active_tab_text_selection_manager() else {
            if note_hover_changed {
                cx.notify();
            }
            return;
        };
        let manager = manager_ref.borrow();

        // Get cache page dimensions - these must match the dimensions used in get_char_bounds_for_page
        let Some(cache) = manager.get_page_cache(page_index) else {
            drop(manager);
            let text_hover_changed = self.set_text_hover_hit(page_index, false);
            if text_hover_changed || note_hover_changed {
                cx.notify();
            }
            return;
        };

        let page_width_pt = cache.page_width;
        let _page_height_pt = cache.page_height;

        // Use the same scale as passed to rendering functions
        let scale = page_width_screen / page_width_pt;

        let is_over_text = cache
            .hit_test_char_at_screen_position(
                local_x,
                local_y,
                (0.0, 0.0, page_width_screen, page_height_screen), // Relative to page container
                scale,
            )
            .is_some();
        let is_selecting = manager.is_selecting();
        let char_index = if is_selecting {
            // Keep nearest-character behavior during dragging for smoother range selection.
            cache.find_char_at_screen_position(
                local_x,
                local_y,
                (0.0, 0.0, page_width_screen, page_height_screen), // Relative to page container
                scale,
            )
        } else {
            None
        };

        drop(manager);
        let hover_changed = self.set_text_hover_hit(page_index, is_over_text);

        if !is_selecting {
            if hover_changed || note_hover_changed {
                cx.notify();
            }
            return;
        }

        if let Some(char_index) = char_index {
            if let Some(manager_ref) = self.active_tab_text_selection_manager() {
                manager_ref
                    .borrow_mut()
                    .update_selection(page_index, char_index);
            }
            cx.notify();
        } else if hover_changed || note_hover_changed {
            cx.notify();
        }
    }

    fn handle_text_mouse_up(&mut self, cx: &mut Context<Self>) {
        if let Some(manager_ref) = self.active_tab_text_selection_manager() {
            manager_ref.borrow_mut().end_selection();
        }
        cx.notify();
    }

    fn ensure_page_text_loaded(&self, page_index: usize) {
        // Check if already loaded
        let Some(manager_ref) = self.active_tab_text_selection_manager() else {
            return;
        };

        if manager_ref.borrow().get_page_cache(page_index).is_some() {
            return;
        }

        // Load text synchronously (pdfium is not thread-safe)
        if let Some(path) = self.active_tab_path() {
            match crate::pdf_viewer::utils::load_page_text_for_selection(path, page_index) {
                Ok(Some((page_index, page_width, page_height, chars))) => {
                    let _char_count = chars.len();
                    if let Ok(mut manager) = manager_ref.try_borrow_mut() {
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

    pub(super) fn render_context_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.context_menu_open {
            return None;
        }

        let i18n = self.i18n();
        let position = self.context_menu_position.unwrap_or_default();
        let x: f32 = position.x.into();
        let y: f32 = position.y.into();

        if let Some(tab_id) = self.context_menu_tab_id {
            let tab_count = self.tab_bar.tabs().len();
            let can_close_others = tab_count > 1;
            let can_reveal = self
                .tab_bar
                .tabs()
                .iter()
                .any(|tab| tab.id == tab_id && tab.path.is_some());

            return Some(
                div()
                    .id(("tab-context-menu", tab_id))
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(196.))
                    .v_flex()
                    .gap_1()
                    .popover_style(cx)
                    .p_1()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        Button::new(("tab-close-all", tab_id))
                            .small()
                            .w_full()
                            .label(i18n.close_all_tabs_button)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_context_menu(cx);
                                this.close_all_tabs(cx);
                            })),
                    )
                    .child(
                        Button::new(("tab-close-others", tab_id))
                            .small()
                            .w_full()
                            .disabled(!can_close_others)
                            .label(i18n.close_other_tabs_button)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.close_context_menu(cx);
                                this.close_other_tabs(tab_id, cx);
                            })),
                    )
                    .child(
                        Button::new(("tab-reveal", tab_id))
                            .small()
                            .w_full()
                            .disabled(!can_reveal)
                            .label(i18n.reveal_in_file_manager_button)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.reveal_tab_in_file_manager(tab_id);
                                this.close_context_menu(cx);
                            })),
                    )
                    .into_any_element(),
            );
        }

        if let Some(note_id) = self.context_menu_note_id {
            let note = self.markdown_note_by_id(note_id);
            let note_markdown = note
                .as_ref()
                .map(|entry| entry.markdown.clone())
                .unwrap_or_default();
            return Some(
                div()
                    .id(("context-menu-note", note_id))
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(220.))
                    .v_flex()
                    .gap_1()
                    .popover_style(cx)
                    .p_1()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        Button::new(("note-edit", note_id))
                            .small()
                            .w_full()
                            .label(i18n.edit_markdown_note_button)
                            .disabled(note.is_none())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.close_context_menu(cx);
                                this.open_markdown_note_editor_for_edit(note_id, window, cx);
                            })),
                    )
                    .child(
                        Button::new(("note-copy", note_id))
                            .small()
                            .w_full()
                            .label(i18n.copy_markdown_note_button)
                            .disabled(note_markdown.trim().is_empty())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !note_markdown.trim().is_empty() {
                                    let _ = super::copy_to_clipboard(&note_markdown);
                                }
                                this.close_context_menu(cx);
                            })),
                    )
                    .child(
                        Button::new(("note-delete", note_id))
                            .small()
                            .w_full()
                            .label(i18n.delete_markdown_note_button)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.delete_markdown_note_by_id(note_id, cx);
                                this.close_context_menu(cx);
                            })),
                    )
                    .into_any_element(),
            );
        }

        let note_anchor = self.context_menu_note_anchor;
        let has_text_selection = self.has_text_selection();
        Some(
            div()
                .id("context-menu")
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(220.))
                .v_flex()
                .gap_1()
                .popover_style(cx)
                .p_1()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|_, _: &gpui::MouseDownEvent, _, cx| {
                        crate::debug_log!(
                            "[context_menu] menu container clicked, stopping propagation"
                        );
                        cx.stop_propagation();
                    }),
                )
                .when(has_text_selection, |this| {
                    this.child(
                        Button::new("copy-text")
                            .small()
                            .w_full()
                            .label(i18n.copy_button)
                            .on_click(cx.listener(|this, _, _, cx| {
                                crate::debug_log!("[context_menu] copy button clicked");
                                this.copy_selected_text();
                                this.close_context_menu(cx);
                            })),
                    )
                })
                .child(
                    Button::new("markdown-note-add")
                        .small()
                        .w_full()
                        .label(i18n.add_markdown_note_here_button)
                        .disabled(note_anchor.is_none())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let Some(anchor) = note_anchor else {
                                return;
                            };
                            this.close_context_menu(cx);
                            this.open_markdown_note_editor_for_new(anchor, window, cx);
                        })),
                )
                .into_any_element(),
        )
    }
}
