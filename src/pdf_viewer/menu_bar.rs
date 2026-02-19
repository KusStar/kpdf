use crate::icons;

use super::PdfViewer;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::popover::Popover;
use gpui_component::{button::*, *};

impl PdfViewer {
    pub(super) fn render_menu_bar(
        &self,
        page_count: usize,
        current_page_num: usize,
        zoom_label: SharedString,
        show_navigation: bool,
        show_zoom: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let i18n = self.i18n();
        let active_page = self.active_tab_active_page();
        let bookmark_popup_open = self.bookmark_popup_open;
        let bookmark_scope = self.bookmark_scope;
        let bookmarks = self.bookmarks_for_scope(bookmark_scope);
        let bookmark_popup_list_scroll = self.bookmark_popup_list_scroll.clone();

        div()
            .id("title-nav-bar")
            .h_full()
            .flex_1()
            .min_w(px(0.))
            .px_1()
            .flex()
            .items_center()
            .justify_between()
            .when(!show_navigation, |this| this.justify_end())
            .when(show_navigation, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(page_count > 0, |this| {
                            this.child(
                                Button::new("first-page")
                                    .ghost()
                                    .small()
                                    .disabled(active_page == 0)
                                    .icon(
                                        Icon::new(icons::IconName::ChevronFirst)
                                            .text_color(cx.theme().foreground),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_page(0, cx);
                                    })),
                            )
                            .child(
                                Button::new("prev-page")
                                    .ghost()
                                    .small()
                                    .disabled(active_page == 0)
                                    .icon(
                                        Icon::new(icons::IconName::ChevronLeft)
                                            .text_color(cx.theme().foreground),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.prev_page(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{} / {}", current_page_num, page_count)),
                            )
                            .child(
                                Button::new("next-page")
                                    .ghost()
                                    .small()
                                    .disabled(active_page + 1 >= page_count)
                                    .icon(
                                        Icon::new(icons::IconName::ChevronRight)
                                            .text_color(cx.theme().foreground),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.next_page(cx);
                                    })),
                            )
                            .child(
                                Button::new("last-page")
                                    .ghost()
                                    .small()
                                    .disabled(active_page + 1 >= page_count)
                                    .icon(
                                        Icon::new(icons::IconName::ChevronLast)
                                            .text_color(cx.theme().foreground),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_page(page_count.saturating_sub(1), cx);
                                    })),
                            )
                        }),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Popover::new("bookmark-popover")
                            .anchor(Corner::TopLeft)
                            .appearance(false)
                            .overlay_closable(false)
                            .open(bookmark_popup_open)
                            .trigger(
                                Button::new("bookmark-add")
                                    .ghost()
                                    .small()
                                    .icon(
                                        Icon::new(icons::IconName::Bookmark)
                                            .size_4()
                                            .text_color(cx.theme().foreground),
                                    )
                                    .on_hover({
                                        let viewer = cx.entity();
                                        move |hovered, _, cx| {
                                            let _ = viewer.update(cx, |this, cx| {
                                                this.set_bookmark_popup_trigger_hovered(
                                                    *hovered, cx,
                                                );
                                            });
                                        }
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.add_current_page_bookmark_and_open(cx);
                                    })),
                            )
                            .content({
                                let viewer = cx.entity();
                                let i18n = i18n;
                                let bookmarks = bookmarks.clone();
                                move |_, _window, cx| {
                                    Self::render_bookmark_popup_panel(
                                        "bookmark-popup",
                                        i18n,
                                        viewer.clone(),
                                        bookmark_scope,
                                        bookmarks.clone(),
                                        &bookmark_popup_list_scroll,
                                        cx,
                                    )
                                }
                            }),
                    )
                    .when(show_zoom, |this| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new("zoom-out")
                                        .ghost()
                                        .small()
                                        .icon(
                                            Icon::new(icons::IconName::Minus)
                                                .text_color(cx.theme().foreground),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.zoom_out(cx);
                                        })),
                                )
                                .child(
                                    Button::new("zoom-reset")
                                        .ghost()
                                        .small()
                                        .label(i18n.zoom_reset_button)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.zoom_reset(cx);
                                        })),
                                )
                                .child(
                                    Button::new("zoom-in")
                                        .ghost()
                                        .small()
                                        .icon(
                                            Icon::new(icons::IconName::Plus)
                                                .text_color(cx.theme().foreground),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.zoom_in(cx);
                                        })),
                                )
                                .child(
                                    div()
                                        .min_w(px(50.))
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(zoom_label),
                                ),
                        )
                    }),
            )
    }
}
