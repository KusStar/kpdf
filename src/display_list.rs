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
                                        .child("点击上方“打开”选择 PDF"),
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
                                            visible_range
                                                .map(|ix| {
                                                    let Some(page) = viewer.pages.get(ix) else {
                                                        return div().into_any_element();
                                                    };
                                                    let display_base_width =
                                                        viewer.display_base_width(_window);
                                                    let (_, display_height) =
                                                        viewer.display_card_size(
                                                            page,
                                                            display_base_width,
                                                        );
                                                    div()
                                                        .id(("display-row", ix))
                                                        .w_full()
                                                        .h_full()
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .h(px(display_height))
                                                                .relative()
                                                                .overflow_hidden()
                                                                .bg(cx.theme().background)
                                                                .when_some(
                                                                    page.display_image.clone(),
                                                                    |this, display_image| {
                                                                        this.child(
                                                                            img(display_image)
                                                                                .size_full()
                                                                                .object_fit(
                                                                                    ObjectFit::Contain,
                                                                                ),
                                                                        )
                                                                    },
                                                                )
                                                                .when(page.display_image.is_none(), |this| {
                                                                    this.child(
                                                                        div()
                                                                            .size_full()
                                                                            .v_flex()
                                                                            .items_center()
                                                                            .justify_center()
                                                                            .gap_2()
                                                                            .text_color(
                                                                                cx.theme()
                                                                                    .muted_foreground,
                                                                            )
                                                                            .when(page.display_failed, |this| {
                                                                                this.child(
                                                                                    Icon::new(
                                                                                        IconName::File,
                                                                                    )
                                                                                    .size_8()
                                                                                    .text_color(
                                                                                        cx.theme()
                                                                                            .muted_foreground,
                                                                                    ),
                                                                                )
                                                                                .child(
                                                                                    div()
                                                                                        .text_xs()
                                                                                        .child(
                                                                                            "页面渲染失败",
                                                                                        ),
                                                                                )
                                                                            })
                                                                            .when(!page.display_failed, |this| {
                                                                                this.child(
                                                                                    spinner::Spinner::new()
                                                                                        .large()
                                                                                        .icon(
                                                                                            Icon::new(
                                                                                                IconName::LoaderCircle,
                                                                                            ),
                                                                                        )
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
                                                                        .left_2()
                                                                        .top_2()
                                                                        .px_2()
                                                                        .py_1()
                                                                        .rounded_md()
                                                                        .bg(
                                                                            cx.theme()
                                                                                .background
                                                                                .opacity(0.9),
                                                                        )
                                                                        .text_xs()
                                                                        .font_medium()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .child(format!(
                                                                            "第 {} 页",
                                                                            page.index + 1
                                                                        )),
                                                                ),
                                                        )
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
}
