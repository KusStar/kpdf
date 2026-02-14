use super::PdfViewer;
use super::utils::display_file_name;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::popover::Popover;
use gpui_component::{button::*, *};
use std::path::PathBuf;

impl PdfViewer {
    pub(super) fn render_menu_bar(
        &self,
        page_count: usize,
        current_page_num: usize,
        recent_popup_open: bool,
        recent_files: Vec<PathBuf>,
        zoom_label: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .h_10()
            .w_full()
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Popover::new("open-pdf-popover")
                            .anchor(Corner::TopLeft)
                            .appearance(false)
                            .overlay_closable(false)
                            .open(recent_popup_open)
                            .trigger(
                                Button::new("open-pdf")
                                    .small()
                                    .icon(
                                        Icon::new(IconName::FolderOpen)
                                            .text_color(cx.theme().foreground),
                                    )
                                    .label("打开")
                                    .on_hover({
                                        let viewer = cx.entity();
                                        move |hovered, _, cx| {
                                            let _ = viewer.update(cx, |this, cx| {
                                                this.set_recent_popup_trigger_hovered(*hovered, cx);
                                            });
                                        }
                                    }),
                            )
                            .content({
                                let viewer = cx.entity();
                                let recent_files = recent_files.clone();
                                move |_, _window, cx| {
                                    div()
                                        .id("open-pdf-popup")
                                        .relative()
                                        .top(px(-1.))
                                        .w(px(340.))
                                        .v_flex()
                                        .gap_1()
                                        .popover_style(cx)
                                        .p_2()
                                        .on_hover({
                                            let viewer = viewer.clone();
                                            move |hovered, _, cx| {
                                                let _ = viewer.update(cx, |this, cx| {
                                                    this.set_recent_popup_panel_hovered(*hovered, cx);
                                                });
                                            }
                                        })
                                        .child(
                                            Button::new("open-pdf-dialog")
                                                .small()
                                                .w_full()
                                                .icon(
                                                    Icon::new(IconName::FolderOpen)
                                                        .text_color(cx.theme().foreground),
                                                )
                                                .label("选择文件...")
                                                .on_click({
                                                    let viewer = viewer.clone();
                                                    move |_, window, cx| {
                                                        let _ = viewer.update(cx, |this, cx| {
                                                            this.close_recent_popup(cx);
                                                            this.open_pdf_dialog(window, cx);
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(div().h(px(1.)).my_1().bg(cx.theme().border))
                                        .when(recent_files.is_empty(), |this| {
                                            this.child(
                                                div()
                                                    .px_2()
                                                    .py_1()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("暂无最近文件"),
                                            )
                                        })
                                        .when(!recent_files.is_empty(), |this| {
                                            this.children(
                                                recent_files
                                                    .iter()
                                                    .enumerate()
                                                    .map(|(ix, path)| {
                                                        let path = path.clone();
                                                        let file_name = display_file_name(&path);
                                                        let path_text = path.display().to_string();
                                                        div()
                                                            .id(("recent-pdf", ix))
                                                            .w_full()
                                                            .rounded_md()
                                                            .px_2()
                                                            .py_1()
                                                            .cursor_pointer()
                                                            .hover(|this| {
                                                                this.bg(
                                                                    cx.theme()
                                                                        .secondary
                                                                        .opacity(0.6),
                                                                )
                                                            })
                                                            .active(|this| {
                                                                this.bg(
                                                                    cx.theme()
                                                                        .secondary
                                                                        .opacity(0.9),
                                                                )
                                                            })
                                                            .child(
                                                                div()
                                                                    .w_full()
                                                                    .v_flex()
                                                                    .items_start()
                                                                    .gap_1()
                                                                    .child(
                                                                        div()
                                                                            .w_full()
                                                                            .whitespace_normal()
                                                                            .text_sm()
                                                                            .text_color(
                                                                                cx.theme()
                                                                                    .popover_foreground,
                                                                            )
                                                                            .child(file_name),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .w_full()
                                                                            .whitespace_normal()
                                                                            .text_xs()
                                                                            .text_color(
                                                                                cx.theme()
                                                                                    .muted_foreground,
                                                                            )
                                                                            .child(path_text),
                                                                    ),
                                                            )
                                                            .on_click({
                                                                let viewer = viewer.clone();
                                                                move |_, _, cx| {
                                                                    let _ =
                                                                        viewer.update(cx, |this, cx| {
                                                                            this.close_recent_popup(cx);
                                                                            this.open_recent_pdf(
                                                                                path.clone(),
                                                                                cx,
                                                                            );
                                                                        });
                                                                }
                                                            })
                                                            .into_any_element()
                                                    })
                                                    .collect::<Vec<_>>(),
                                            )
                                        })
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(page_count > 0, |this| {
                                this.child(
                                    Button::new("prev-page")
                                        .ghost()
                                        .small()
                                        .disabled(self.active_page == 0)
                                        .icon(
                                            Icon::new(IconName::ChevronLeft)
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
                                        .disabled(self.active_page + 1 >= page_count)
                                        .icon(
                                            Icon::new(IconName::ChevronRight)
                                                .text_color(cx.theme().foreground),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.next_page(cx);
                                        })),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("zoom-out")
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Minus).text_color(cx.theme().foreground))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.zoom_out(cx);
                            })),
                    )
                    .child(
                        Button::new("zoom-reset")
                            .ghost()
                            .small()
                            .label("适配")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.zoom_reset(cx);
                            })),
                    )
                    .child(
                        Button::new("zoom-in")
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Plus).text_color(cx.theme().foreground))
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
    }
}
