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
        let selection_rects = self.get_selection_rects_for_page(page_index, scale);

        // Get page info for coordinate conversion
        let page_height_pt = page.height_pt;
        let page_width_pt = page.width_pt;

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
                                    move |this, event: &gpui::MouseDownEvent, _window, cx| {
                                        let local_x = f32::from(event.position.x);
                                        let local_y = f32::from(event.position.y);

                                        eprintln!("[mouse_down] local: ({}, {})", local_x, local_y);

                                        this.handle_text_mouse_down(
                                            page_index,
                                            local_x,
                                            local_y,
                                            page_width_pt,
                                            page_height_pt,
                                            scale,
                                            cx,
                                        );
                                    },
                                ),
                            )
                            .on_mouse_move(cx.listener(
                                move |this, event: &gpui::MouseMoveEvent, _window, cx| {
                                    let local_x = f32::from(event.position.x);
                                    let local_y = f32::from(event.position.y);

                                    this.handle_text_mouse_move(
                                        page_index,
                                        local_x,
                                        local_y,
                                        page_width_pt,
                                        page_height_pt,
                                        scale,
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
                                    .bg(gpui::rgba(0x3390FF40)) // Semi-transparent blue highlight
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
        scale: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        let manager = self.text_selection_manager.borrow();
        let Some(rects) = manager.get_selection_rects(page_index) else {
            eprintln!("[selection_rects] No selection for page {}", page_index);
            return Vec::new();
        };

        let Some(page) = self.pages.get(page_index) else {
            eprintln!("[selection_rects] No page {} found", page_index);
            return Vec::new();
        };

        eprintln!("[selection_rects] Page {} has {} rects, scale={}, page_height={}", 
            page_index, rects.len(), scale, page.height_pt);

        let result: Vec<(f32, f32, f32, f32)> = rects
            .into_iter()
            .map(|(left, top, right, bottom)| {
                // Convert from PDF coordinates to screen coordinates
                // PDF: origin at bottom-left, y increases upward
                // Screen: origin at top-left, y increases downward
                let screen_left = left * scale;
                let screen_right = right * scale;
                // In PDF: top > bottom (top is higher y value)
                // After conversion: screen_top < screen_bottom (top is smaller y on screen)
                let screen_top = (page.height_pt - top) * scale;
                let screen_bottom = (page.height_pt - bottom) * scale;

                // screen_top should be smaller (higher on screen), screen_bottom larger (lower on screen)
                // Return (left, top, right, bottom) where top < bottom for positive height
                let (final_top, final_bottom) = if screen_top < screen_bottom {
                    (screen_top, screen_bottom)
                } else {
                    (screen_bottom, screen_top)
                };

                eprintln!("[selection_rects] PDF({},{},{},{}) -> Screen({},{},{},{})", 
                    left, top, right, bottom, screen_left, final_top, screen_right, final_bottom);

                (screen_left, final_top, screen_right, final_bottom)
            })
            .collect();

        result
    }

    fn handle_text_mouse_down(
        &mut self,
        page_index: usize,
        local_x: f32,
        local_y: f32,
        page_width_pt: f32,
        page_height_pt: f32,
        scale: f32,
        cx: &mut Context<Self>,
    ) {
        // Ensure text is loaded before handling mouse down
        self.ensure_page_text_loaded(page_index);

        let manager = self.text_selection_manager.borrow();

        // Convert local coordinates to PDF coordinates
        // The page is displayed with ObjectFit::Contain which maintains aspect ratio
        // We need to account for any letterboxing/pillarboxing
        let aspect_ratio_page = page_width_pt / page_height_pt;
        let aspect_ratio_display = local_x / local_y;

        // Calculate the actual content area within the display
        let (content_width, content_height, offset_x, offset_y) =
            if aspect_ratio_page > aspect_ratio_display {
                // Letterbox: full width, centered height
                let h = page_width_pt / aspect_ratio_page;
                let y_offset = (page_height_pt - h) / 2.0;
                (page_width_pt, h, 0.0, y_offset)
            } else {
                // Pillarbox: full height, centered width
                let w = page_height_pt * aspect_ratio_page;
                let x_offset = (page_width_pt - w) / 2.0;
                (w, page_height_pt, x_offset, 0.0)
            };

        // Scale to screen coordinates
        let content_width_screen = content_width * scale;
        let content_height_screen = content_height * scale;
        let offset_x_screen = offset_x * scale;
        let offset_y_screen = offset_y * scale;

        // Check if click is within the content area
        if local_x < offset_x_screen
            || local_x > offset_x_screen + content_width_screen
            || local_y < offset_y_screen
            || local_y > offset_y_screen + content_height_screen
        {
            eprintln!("[mouse_down] Click outside content area");
            drop(manager);
            self.text_selection_manager.borrow_mut().clear_selection();
            cx.notify();
            return;
        }

        // Convert to normalized coordinates (0-1)
        let norm_x = (local_x - offset_x_screen) / content_width_screen;
        let norm_y = (local_y - offset_y_screen) / content_height_screen;

        // Convert to PDF coordinates
        let pdf_x = norm_x * page_width_pt;
        let pdf_y = page_height_pt - (norm_y * page_height_pt); // Flip Y

        eprintln!(
            "[mouse_down] local=({}, {}), pdf=({}, {})",
            local_x, local_y, pdf_x, pdf_y
        );

        // Find character at position using the cached page data
        let char_index = if let Some(cache) = manager.get_page_cache(page_index) {
            let idx = cache.find_char_at_position(pdf_x, pdf_y);
            if let Some(i) = idx {
                if let Some(char_info) = cache.chars.get(i) {
                    eprintln!("[mouse_down] Found char[{}] = '{}' at pdf({},{})", 
                        i, char_info.text, char_info.left, char_info.top);
                }
            }
            idx
        } else {
            None
        };

        drop(manager); // Release borrow before mutable borrow

        if let Some(char_index) = char_index {
            eprintln!("[mouse_down] Selected char index: {}", char_index);
            self.text_selection_manager
                .borrow_mut()
                .start_selection(page_index, char_index);
        } else {
            eprintln!("[mouse_down] No character found at position");
            self.text_selection_manager.borrow_mut().clear_selection();
        }
        cx.notify();
    }

    fn handle_text_mouse_move(
        &mut self,
        page_index: usize,
        local_x: f32,
        local_y: f32,
        page_width_pt: f32,
        page_height_pt: f32,
        scale: f32,
        cx: &mut Context<Self>,
    ) {
        // Ensure text is loaded before handling mouse move
        self.ensure_page_text_loaded(page_index);

        let manager = self.text_selection_manager.borrow();

        if !manager.is_selecting() {
            return;
        }

        // Same coordinate conversion as mouse_down
        let aspect_ratio_page = page_width_pt / page_height_pt;
        let aspect_ratio_display = local_x / local_y;

        let (content_width, content_height, offset_x, offset_y) =
            if aspect_ratio_page > aspect_ratio_display {
                let h = page_width_pt / aspect_ratio_page;
                let y_offset = (page_height_pt - h) / 2.0;
                (page_width_pt, h, 0.0, y_offset)
            } else {
                let w = page_height_pt * aspect_ratio_page;
                let x_offset = (page_width_pt - w) / 2.0;
                (w, page_height_pt, x_offset, 0.0)
            };

        let content_width_screen = content_width * scale;
        let content_height_screen = content_height * scale;
        let offset_x_screen = offset_x * scale;
        let offset_y_screen = offset_y * scale;

        // Clamp to content bounds
        let local_x = local_x.clamp(offset_x_screen, offset_x_screen + content_width_screen);
        let local_y = local_y.clamp(offset_y_screen, offset_y_screen + content_height_screen);

        let norm_x = (local_x - offset_x_screen) / content_width_screen;
        let norm_y = (local_y - offset_y_screen) / content_height_screen;

        let pdf_x = norm_x * page_width_pt;
        let pdf_y = page_height_pt - (norm_y * page_height_pt);

        let char_index = if let Some(cache) = manager.get_page_cache(page_index) {
            cache.find_char_at_position(pdf_x, pdf_y)
        } else {
            None
        };

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
        eprintln!(
            "[text] Will load text for page {} on user interaction",
            page_index
        );
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
                    let char_count = chars.len();
                    if let Ok(mut manager) = self.text_selection_manager.try_borrow_mut() {
                        manager.load_cached_text(page_index, page_width, page_height, chars);
                        eprintln!(
                            "[text] Loaded {} characters for page {}",
                            char_count, page_index
                        );
                    }
                }
                Ok(None) => {
                    eprintln!("[text] No text found on page {}", page_index);
                }
                Err(e) => {
                    eprintln!("[text] Failed to load text for page {}: {}", page_index, e);
                }
            }
        }
    }
}
