use super::{PdfViewer, SIDEBAR_WIDTH};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use gpui_component::*;
use std::rc::Rc;

impl PdfViewer {
    pub(super) fn render_thumbnail_panel(
        &self,
        page_count: usize,
        thumbnail_sizes: Rc<Vec<gpui::Size<Pixels>>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let i18n = self.i18n();
        let _active_page = self.active_tab_active_page();

        div()
            .h_full()
            .w(px(SIDEBAR_WIDTH))
            .flex_none()
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .bg(cx.theme().sidebar)
            .overflow_hidden()
            .when(page_count == 0, |this| {
                this.child(
                    div()
                        .h_full()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(i18n.no_pages()),
                )
            })
            .when(page_count > 0, |this| {
                this.child(
                    div()
                        .relative()
                        .size_full()
                        .child(
                            v_virtual_list(
                                cx.entity(),
                                "thumb-virtual-list",
                                thumbnail_sizes.clone(),
                                move |viewer, visible_range, _window, cx| {
                                    let target_width = viewer.thumbnail_target_width(_window);
                                    viewer.request_thumbnail_load_for_visible_range(
                                        visible_range.clone(),
                                        target_width,
                                        cx,
                                    );
                                    
                                    let active_page = viewer.active_tab_active_page();
                                    
                                    visible_range
                                        .map(|ix| {
                                            let Some(pages) = viewer.active_tab_pages() else {
                                                return div().into_any_element();
                                            };
                                            let Some(page) = pages.get(ix) else {
                                                return div().into_any_element();
                                            };
                                            let (_, thumb_height) = viewer.thumbnail_card_size(page);
                                            let is_selected = ix == active_page;
                                            div()
                                                .id(("thumb-row", ix))
                                                .px_2()
                                                .py_1()
                                                .rounded_md()
                                                .when(is_selected, |this| {
                                                    this.bg(cx.theme().secondary.opacity(0.55))
                                                })
                                                .hover(|this| {
                                                    this.bg(cx.theme().secondary.opacity(0.35))
                                                })
                                                .active(|this| {
                                                    this.bg(cx.theme().secondary.opacity(0.6))
                                                })
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .h(px(thumb_height))
                                                        .relative()
                                                        .overflow_hidden()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(if is_selected {
                                                            cx.theme().foreground
                                                        } else {
                                                            cx.theme().sidebar_border
                                                        })
                                                        .bg(cx.theme().background)
                                                        .when_some(
                                                            page.thumbnail_image.clone(),
                                                            |this, thumbnail_image| {
                                                                this.child(
                                                                    img(thumbnail_image)
                                                                        .size_full()
                                                                        .object_fit(
                                                                            ObjectFit::Contain,
                                                                        ),
                                                                )
                                                            },
                                                        )
                                                        .when(page.thumbnail_image.is_none(), |this| {
                                                            this.child(
                                                                div()
                                                                    .size_full()
                                                                    .v_flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .gap_2()
                                                                    .text_color(
                                                                        cx.theme().muted_foreground,
                                                                    )
                                                                    .when(page.thumbnail_failed, |this| {
                                                                        this.child(
                                                                            Icon::new(IconName::File)
                                                                                .size_5()
                                                                                .text_color(
                                                                                    cx.theme()
                                                                                        .muted_foreground,
                                                                                ),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .child(
                                                                                    i18n.thumbnail_render_failed(),
                                                                                ),
                                                                        )
                                                                    })
                                                                    .when(!page.thumbnail_failed, |this| {
                                                                        this.child(
                                                                            spinner::Spinner::new()
                                                                                .large()
                                                                                .icon(Icon::new(
                                                                                    IconName::LoaderCircle,
                                                                                ))
                                                                                .color(
                                                                                    cx.theme()
                                                                                        .muted_foreground,
                                                                                ),
                                                                        )
                                                                    }),
                                                            )
                                                        })
                                                        .child(
                                                            div()
                                                                .absolute()
                                                                .left_1()
                                                                .top_1()
                                                                .px_1()
                                                                .rounded_sm()
                                                                .bg(
                                                                    cx.theme()
                                                                        .background
                                                                        .opacity(0.9),
                                                                )
                                                                .text_xs()
                                                                .font_medium()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                )
                                                                .child(format!("{}", page.index + 1)),
                                                        ),
                                                )
                                                .cursor_pointer()
                                                .on_click(cx.listener(
                                                    move |this, _, _, cx| {
                                                        this.select_page(ix, cx);
                                                    },
                                                ))
                                                .into_any_element()
                                        })
                                        .collect::<Vec<_>>()
                                },
                            )
                            .track_scroll(self.active_tab_thumbnail_scroll().unwrap())
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
                                    Scrollbar::vertical(self.active_tab_thumbnail_scroll().unwrap())
                                        .scrollbar_show(ScrollbarShow::Always),
                                ),
                        )
                        .into_any_element(),
                )
            })
    }
}
