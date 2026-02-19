use crate::icons;

use super::PdfViewer;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
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

        div()
            .id("title-nav-bar")
            .h_full()
            .flex_1()
            .min_w(px(0.))
            .px_1()
            .flex()
            .items_center()
            .justify_between()
            .when(!show_navigation && show_zoom, |this| this.justify_end())
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
                                .label(i18n.zoom_reset_button())
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
            })
    }
}
