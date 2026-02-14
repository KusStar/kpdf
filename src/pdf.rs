mod utils;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{button::*, *};
use pdf_rs::objects::ObjRefTuple;
use std::path::PathBuf;
use std::rc::Rc;

const ZOOM_MIN: f32 = 0.6;
const ZOOM_MAX: f32 = 2.5;
const ZOOM_STEP: f32 = 0.1;
const THUMB_ROW_HEIGHT: f32 = 56.0;
const PREVIEW_ROW_PADDING: f32 = 16.0;

use self::utils::{display_file_name, load_document_summary};

#[derive(Clone)]
struct PageSummary {
    index: usize,
    object_ref: ObjRefTuple,
    width_pt: f32,
    height_pt: f32,
}

pub struct PdfViewer {
    path: Option<PathBuf>,
    pages: Vec<PageSummary>,
    selected_page: usize,
    zoom: f32,
    status: SharedString,
    thumbnail_scroll: VirtualListScrollHandle,
    preview_scroll: VirtualListScrollHandle,
}

impl PdfViewer {
    pub fn new() -> Self {
        Self {
            path: None,
            pages: Vec::new(),
            selected_page: 0,
            zoom: 1.0,
            status: "打开一个 PDF 文件".into(),
            thumbnail_scroll: VirtualListScrollHandle::new(),
            preview_scroll: VirtualListScrollHandle::new(),
        }
    }

    fn open_pdf_dialog(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.status = "选择 PDF 文件...".into();
        cx.notify();

        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open PDF".into()),
        });

        cx.spawn(async move |view, cx| {
            let picker_result = picker.await;
            match picker_result {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next() {
                        let parsed = load_document_summary(&path);
                        let _ = view.update(cx, |this, cx| match parsed {
                            Ok(mut pages) => {
                                pages.sort_by_key(|p| p.index);
                                this.path = Some(path.clone());
                                this.pages = pages;
                                this.selected_page = 0;
                                this.zoom = 1.0;
                                this.status =
                                    format!("{} 页 | {}", this.pages.len(), display_file_name(&path))
                                        .into();
                                if !this.pages.is_empty() {
                                    this.thumbnail_scroll.scroll_to_item(0, ScrollStrategy::Top);
                                    this.preview_scroll.scroll_to_item(0, ScrollStrategy::Top);
                                }
                                cx.notify();
                            }
                            Err(err) => {
                                this.path = Some(path.clone());
                                this.pages.clear();
                                this.selected_page = 0;
                                this.status =
                                    format!("加载失败: {} ({})", display_file_name(&path), err)
                                        .into();
                                cx.notify();
                            }
                        });
                    } else {
                        let _ = view.update(cx, |this, cx| {
                            this.status = "未选择文件".into();
                            cx.notify();
                        });
                    }
                }
                Ok(Ok(None)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.status = "已取消".into();
                        cx.notify();
                    });
                }
                Ok(Err(err)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.status = format!("文件选择失败: {err}").into();
                        cx.notify();
                    });
                }
                Err(err) => {
                    let _ = view.update(cx, |this, cx| {
                        this.status = format!("文件选择失败: {err}").into();
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn select_page(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.pages.len() {
            self.selected_page = index;
            self.sync_scroll_to_selected();
            cx.notify();
        }
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.selected_page > 0 {
            self.select_page(self.selected_page - 1, cx);
        }
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        if self.selected_page + 1 < self.pages.len() {
            self.select_page(self.selected_page + 1, cx);
        }
    }

    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.zoom = (self.zoom + ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
        cx.notify();
    }

    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.zoom = (self.zoom - ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
        cx.notify();
    }

    fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        cx.notify();
    }

    fn sync_scroll_to_selected(&mut self) {
        self.thumbnail_scroll
            .scroll_to_item(self.selected_page, ScrollStrategy::Center);
        self.preview_scroll
            .scroll_to_item(self.selected_page, ScrollStrategy::Center);
    }

    fn thumbnail_item_sizes(&self) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            (0..self.pages.len())
                .map(|_| size(px(0.), px(THUMB_ROW_HEIGHT)))
                .collect(),
        )
    }

    fn preview_card_size(&self, page: &PageSummary) -> (f32, f32) {
        let scale = 0.5 * self.zoom;
        let width = (page.width_pt * scale).clamp(220.0, 1200.0);
        let height = (page.height_pt * scale).clamp(260.0, 1600.0);
        (width, height)
    }

    fn preview_row_height(&self, page: &PageSummary) -> f32 {
        let (_, height) = self.preview_card_size(page);
        height + PREVIEW_ROW_PADDING * 2.0
    }

    fn preview_item_sizes(&self) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            self.pages
                .iter()
                .map(|page| size(px(0.), px(self.preview_row_height(page))))
                .collect(),
        )
    }
}

impl Render for PdfViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(cx.theme().font_size);

        let page_count = self.pages.len();
        let current_page_num = if page_count == 0 {
            0
        } else {
            self.selected_page + 1
        };
        let file_name = self
            .path
            .as_ref()
            .map(|p| display_file_name(p))
            .unwrap_or_else(|| "未打开文件".to_string());
        let zoom_label: SharedString = format!("{:.0}%", self.zoom * 100.0).into();
        let thumbnail_sizes = self.thumbnail_item_sizes();
        let preview_sizes = self.preview_item_sizes();

        window_border().child(
            div()
                .v_flex()
                .size_full()
                .bg(cx.theme().background)
                .child(
                    div()
                        .h(px(34.))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(cx.theme().title_bar_border)
                        .bg(cx.theme().title_bar)
                        .child(
                            div()
                                .h_full()
                                .flex_1()
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_2()
                                .window_control_area(WindowControlArea::Drag)
                                .child(
                                    Icon::new(IconName::File)
                                        .size_4()
                                        .text_color(cx.theme().foreground),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(cx.theme().foreground)
                                        .child("KPDF"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(file_name),
                                ),
                        )
                        .child(
                            div()
                                .h_full()
                                .pr_1()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new("window-minimize")
                                        .ghost()
                                        .small()
                                        .icon(
                                            Icon::new(IconName::WindowMinimize)
                                                .text_color(cx.theme().foreground),
                                        )
                                        .on_click(|_, window, _| window.minimize_window()),
                                )
                                .child(
                                    Button::new("window-maximize")
                                        .ghost()
                                        .small()
                                        .icon(
                                            Icon::new(if window.is_maximized() {
                                                IconName::WindowRestore
                                            } else {
                                                IconName::WindowMaximize
                                            })
                                            .text_color(cx.theme().foreground),
                                        )
                                        .on_click(|_, window, _| window.zoom_window()),
                                )
                                .child(
                                    Button::new("window-close")
                                        .ghost()
                                        .small()
                                        .icon(
                                            Icon::new(IconName::WindowClose)
                                                .text_color(cx.theme().foreground),
                                        )
                                        .on_click(|_, window, _| window.remove_window()),
                                ),
                        ),
                )
                .child(
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
                                    Button::new("open-pdf")
                                        .small()
                                        .icon(
                                            Icon::new(IconName::FolderOpen)
                                                .text_color(cx.theme().foreground),
                                        )
                                        .label("打开")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_pdf_dialog(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("prev-page")
                                        .ghost()
                                        .small()
                                        .disabled(page_count == 0 || self.selected_page == 0)
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
                                        .disabled(
                                            page_count == 0 || self.selected_page + 1 >= page_count,
                                        )
                                        .icon(
                                            Icon::new(IconName::ChevronRight)
                                                .text_color(cx.theme().foreground),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.next_page(cx);
                                        })),
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
                                        .icon(
                                            Icon::new(IconName::Minus)
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
                                        .label("适配")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.zoom_reset(cx);
                                        })),
                                )
                                .child(
                                    Button::new("zoom-in")
                                        .ghost()
                                        .small()
                                        .icon(
                                            Icon::new(IconName::Plus)
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
                        ),
                )
                .child(
                    div()
                        .h_full()
                        .w_full()
                        .flex()
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(px(228.))
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
                                            .child("暂无页面"),
                                    )
                                })
                                .when(page_count > 0, |this| {
                                    this.child(
                                        v_virtual_list(
                                            cx.entity(),
                                            "thumb-virtual-list",
                                            thumbnail_sizes.clone(),
                                            move |viewer, visible_range, _window, cx| {
                                                visible_range
                                                    .map(|ix| {
                                                        let Some(_page) = viewer.pages.get(ix) else {
                                                            return div().into_any_element();
                                                        };
                                                        let is_selected = ix == viewer.selected_page;
                                                        div()
                                                            .px_2()
                                                            .py_1()
                                                            .child(
                                                                Button::new(("thumb", ix))
                                                                    .ghost()
                                                                    .small()
                                                                    .w_full()
                                                                    .selected(is_selected)
                                                                    .child(
                                                                        div()
                                                                            .w_full()
                                                                            .v_flex()
                                                                            .items_start()
                                                                            .gap_1()
                                                                            .child(
                                                                                div()
                                                                                    .text_sm()
                                                                                    .font_medium()
                                                                                    .text_color(
                                                                                        cx.theme()
                                                                                            .sidebar_foreground,
                                                                                    )
                                                                                    .child(format!(
                                                                                        "第 {} 页",
                                                                                        ix + 1
                                                                                    )),
                                                                            ),
                                                                    )
                                                                    .on_click(cx.listener(
                                                                        move |this, _, _, cx| {
                                                                            this.select_page(ix, cx);
                                                                        },
                                                                    )),
                                                            )
                                                            .into_any_element()
                                                    })
                                                    .collect::<Vec<_>>()
                                            },
                                        )
                                        .track_scroll(&self.thumbnail_scroll)
                                        .into_any_element(),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .h_full()
                                .flex_1()
                                .v_flex()
                                .overflow_hidden()
                                .bg(cx.theme().muted)
                                .child(
                                    div()
                                        .h_8()
                                        .w_full()
                                        .px_4()
                                        .flex()
                                        .items_center()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(self.status.clone()),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .w_full()
                                        .overflow_hidden()
                                        .when(page_count == 0, |this| {
                                            this.child(
                                                div()
                                                    .size_full()
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
                                                v_virtual_list(
                                                    cx.entity(),
                                                    "preview-virtual-list",
                                                    preview_sizes.clone(),
                                                    move |viewer, visible_range, _window, cx| {
                                                        visible_range
                                                            .map(|ix| {
                                                                let Some(page) = viewer.pages.get(ix)
                                                                else {
                                                                    return div().into_any_element();
                                                                };
                                                                let is_selected =
                                                                    ix == viewer.selected_page;
                                                                let (preview_width, preview_height) =
                                                                    viewer.preview_card_size(page);
                                                                div()
                                                                    .id(("preview-row", ix))
                                                                    .w_full()
                                                                    .h_full()
                                                                    .py(px(PREVIEW_ROW_PADDING))
                                                                    .flex()
                                                                    .justify_center()
                                                                    .child(
                                                                        div()
                                                                            .w(px(preview_width))
                                                                            .h(px(preview_height))
                                                                            .v_flex()
                                                                            .items_center()
                                                                            .justify_center()
                                                                            .gap_2()
                                                                            .rounded_lg()
                                                                            .border_1()
                                                                            .border_color(
                                                                                if is_selected {
                                                                                    cx.theme().accent
                                                                                } else {
                                                                                    cx.theme().border
                                                                                },
                                                                            )
                                                                            .bg(cx.theme().background)
                                                                            .shadow_lg()
                                                                            .child(
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
                                                                                    .text_sm()
                                                                                    .font_semibold()
                                                                                    .text_color(
                                                                                        cx.theme()
                                                                                            .foreground,
                                                                                    )
                                                                                    .child(format!(
                                                                                        "Page {}",
                                                                                        page.index + 1
                                                                                    )),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_xs()
                                                                                    .text_color(
                                                                                        cx.theme()
                                                                                            .muted_foreground,
                                                                                    )
                                                                                    .child(format!(
                                                                                        "{:.0} x {:.0} pt",
                                                                                        page.width_pt,
                                                                                        page.height_pt
                                                                                    )),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_xs()
                                                                                    .text_color(
                                                                                        cx.theme()
                                                                                            .muted_foreground,
                                                                                    )
                                                                                    .child(format!(
                                                                                        "obj {} {} R",
                                                                                        page.object_ref
                                                                                            .0,
                                                                                        page.object_ref
                                                                                            .1
                                                                                    )),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_xs()
                                                                                    .text_color(
                                                                                        cx.theme()
                                                                                            .muted_foreground,
                                                                                    )
                                                                                    .child(
                                                                                        "pdf-rs 解析页面结构，预览为极简占位",
                                                                                    ),
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
                                                .track_scroll(&self.preview_scroll)
                                                .into_any_element(),
                                            )
                                        }),
                                ),
                        ),
                ),
        )
    }
}
