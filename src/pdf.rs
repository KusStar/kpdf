mod utils;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use gpui_component::{button::*, *};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const ZOOM_MIN: f32 = 0.6;
const ZOOM_MAX: f32 = 2.5;
const ZOOM_STEP: f32 = 0.1;
const THUMB_ROW_HEIGHT: f32 = 56.0;
const SIDEBAR_WIDTH: f32 = 228.0;
const DISPLAY_MIN_WIDTH: f32 = 220.0;
const DISPLAY_BATCH_SIZE: usize = 1;
const DISPLAY_MAX_PARALLEL_TASKS: usize = 1;
const DISPLAY_SCROLL_SYNC_DELAY_MS: u64 = 140;

use self::utils::{display_file_name, load_display_images, load_document_summary};

#[derive(Clone)]
struct PageSummary {
    index: usize,
    width_pt: f32,
    height_pt: f32,
    display_image: Option<Arc<RenderImage>>,
    display_render_width: u32,
    display_failed: bool,
}

pub struct PdfViewer {
    path: Option<PathBuf>,
    pages: Vec<PageSummary>,
    selected_page: usize,
    zoom: f32,
    status: SharedString,
    thumbnail_scroll: VirtualListScrollHandle,
    display_scroll: VirtualListScrollHandle,
    display_loading: HashSet<usize>,
    display_inflight_tasks: usize,
    display_epoch: u64,
    last_display_visible_range: Option<std::ops::Range<usize>>,
    last_display_target_width: u32,
    display_scroll_sync_epoch: u64,
    last_display_scroll_offset: Option<Point<Pixels>>,
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
            display_scroll: VirtualListScrollHandle::new(),
            display_loading: HashSet::new(),
            display_inflight_tasks: 0,
            display_epoch: 0,
            last_display_visible_range: None,
            last_display_target_width: DISPLAY_MIN_WIDTH as u32,
            display_scroll_sync_epoch: 0,
            last_display_scroll_offset: None,
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
                        let parsed = cx
                            .background_executor()
                            .spawn({
                                let path = path.clone();
                                async move { load_document_summary(&path) }
                            })
                            .await;
                        let _ = view.update(cx, |this, cx| match parsed {
                            Ok(mut pages) => {
                                pages.sort_by_key(|p| p.index);
                                this.path = Some(path.clone());
                                this.pages = pages;
                                this.selected_page = 0;
                                this.zoom = 1.0;
                                this.display_loading.clear();
                                this.display_inflight_tasks = 0;
                                this.display_epoch = this.display_epoch.wrapping_add(1);
                                this.last_display_visible_range = None;
                                this.status = format!(
                                    "{} 页 | {} | 按需渲染",
                                    this.pages.len(),
                                    display_file_name(&path)
                                )
                                .into();
                                if !this.pages.is_empty() {
                                    this.thumbnail_scroll.scroll_to_item(0, ScrollStrategy::Top);
                                    this.display_scroll.scroll_to_item(0, ScrollStrategy::Top);
                                }
                                cx.notify();
                            }
                            Err(err) => {
                                this.path = Some(path.clone());
                                this.pages.clear();
                                this.selected_page = 0;
                                this.display_loading.clear();
                                this.display_inflight_tasks = 0;
                                this.display_epoch = this.display_epoch.wrapping_add(1);
                                this.last_display_visible_range = None;
                                this.status =
                                    format!("加载失败: {} ({})", display_file_name(&path), err)
                                        .into();
                                cx.notify();
                            }
                        });
                    } else {
                        let _ = view.update(cx, |this, cx| {
                            this.status = "未选择文件".into();
                            this.display_loading.clear();
                            this.display_inflight_tasks = 0;
                            this.display_epoch = this.display_epoch.wrapping_add(1);
                            this.last_display_visible_range = None;
                            cx.notify();
                        });
                    }
                }
                Ok(Ok(None)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.status = "已取消".into();
                        this.display_loading.clear();
                        this.display_inflight_tasks = 0;
                        this.display_epoch = this.display_epoch.wrapping_add(1);
                        this.last_display_visible_range = None;
                        cx.notify();
                    });
                }
                Ok(Err(err)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.status = format!("文件选择失败: {err}").into();
                        this.display_loading.clear();
                        this.display_inflight_tasks = 0;
                        this.display_epoch = this.display_epoch.wrapping_add(1);
                        this.last_display_visible_range = None;
                        cx.notify();
                    });
                }
                Err(err) => {
                    let _ = view.update(cx, |this, cx| {
                        this.status = format!("文件选择失败: {err}").into();
                        this.display_loading.clear();
                        this.display_inflight_tasks = 0;
                        this.display_epoch = this.display_epoch.wrapping_add(1);
                        this.last_display_visible_range = None;
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
        self.display_scroll
            .scroll_to_item(self.selected_page, ScrollStrategy::Center);
    }

    fn schedule_thumbnail_sync_after_display_scroll(&mut self, cx: &mut Context<Self>) {
        self.display_scroll_sync_epoch = self.display_scroll_sync_epoch.wrapping_add(1);
        let sync_epoch = self.display_scroll_sync_epoch;

        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(DISPLAY_SCROLL_SYNC_DELAY_MS))
                .await;

            let _ = view.update(cx, |this, cx| {
                if this.display_scroll_sync_epoch != sync_epoch || this.pages.is_empty() {
                    return;
                }

                let index = this.selected_page.min(this.pages.len().saturating_sub(1));
                this.thumbnail_scroll
                    .scroll_to_item(index, ScrollStrategy::Center);
                cx.notify();
            });
        })
        .detach();
    }

    fn on_display_scroll_offset_changed(&mut self, cx: &mut Context<Self>) {
        let offset = self.display_scroll.offset();
        let has_changed = self
            .last_display_scroll_offset
            .map(|last| last != offset)
            .unwrap_or(false);
        self.last_display_scroll_offset = Some(offset);

        if has_changed && !self.pages.is_empty() {
            self.schedule_thumbnail_sync_after_display_scroll(cx);
        }
    }

    fn thumbnail_item_sizes(&self) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            (0..self.pages.len())
                .map(|_| size(px(0.), px(THUMB_ROW_HEIGHT)))
                .collect(),
        )
    }

    fn display_base_width(&self, window: &Window) -> f32 {
        let viewport_width: f32 = window.viewport_size().width.into();
        (viewport_width - SIDEBAR_WIDTH).max(DISPLAY_MIN_WIDTH)
    }

    fn display_card_size(&self, page: &PageSummary, base_width: f32) -> (f32, f32) {
        let width = (base_width * self.zoom).max(DISPLAY_MIN_WIDTH);
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

    fn display_item_sizes(&self, base_width: f32) -> Rc<Vec<gpui::Size<Pixels>>> {
        Rc::new(
            self.pages
                .iter()
                .map(|page| size(px(0.), px(self.display_row_height(page, base_width))))
                .collect(),
        )
    }

    fn display_target_width(&self, window: &Window) -> u32 {
        let width = self.display_base_width(window) * self.zoom * window.scale_factor();
        width.clamp(1.0, i32::MAX as f32).round() as u32
    }

    fn request_display_load_from_candidates(
        &mut self,
        candidate_order: Vec<usize>,
        target_width: u32,
        cx: &mut Context<Self>,
    ) {
        if candidate_order.is_empty() || self.pages.is_empty() {
            return;
        }

        let Some(path) = self.path.clone() else {
            return;
        };

        if self.display_inflight_tasks >= DISPLAY_MAX_PARALLEL_TASKS {
            return;
        }

        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for ix in candidate_order {
            if !seen.insert(ix) {
                continue;
            }

            let Some(page) = self.pages.get(ix) else {
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
            self.display_loading.insert(*ix);
        }
        self.display_inflight_tasks = self.display_inflight_tasks.saturating_add(1);

        let epoch = self.display_epoch;
        cx.spawn(async move |view, cx| {
            let load_result = cx
                .background_executor()
                .spawn(async move {
                    let loaded = load_display_images(&path, &pending, target_width);
                    (pending, target_width, loaded)
                })
                .await;

            let _ = view.update(cx, |this, cx| {
                this.display_inflight_tasks = this.display_inflight_tasks.saturating_sub(1);
                if this.display_epoch != epoch {
                    return;
                }

                let (requested_indices, loaded_target_width, loaded_result) = load_result;
                let mut loaded_indices = HashSet::new();

                match loaded_result {
                    Ok(images) => {
                        for (ix, image) in images {
                            if let Some(page) = this.pages.get_mut(ix) {
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
                    this.display_loading.remove(&ix);
                    if !loaded_indices.contains(&ix)
                        && let Some(page) = this.pages.get_mut(ix)
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
        if visible_range.is_empty() || self.pages.is_empty() {
            return;
        }

        if self.display_inflight_tasks == 0 && !self.display_loading.is_empty() {
            self.display_loading.clear();
        }

        let next_selected = visible_range.start.min(self.pages.len().saturating_sub(1));
        if self.selected_page != next_selected {
            self.selected_page = next_selected;
            cx.notify();
        }

        self.last_display_visible_range = Some(visible_range.clone());

        self.trim_display_cache(visible_range.clone());

        let mut candidate_order = Vec::with_capacity(visible_range.len());
        candidate_order.extend(visible_range.clone());

        self.request_display_load_from_candidates(candidate_order, target_width, cx);
    }

    fn trim_display_cache(&mut self, visible_range: std::ops::Range<usize>) {
        let _ = visible_range;
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
        self.last_display_target_width = self.display_target_width(window);
        self.on_display_scroll_offset_changed(cx);
        let display_base_width = self.display_base_width(window);
        let thumbnail_sizes = self.thumbnail_item_sizes();
        let display_sizes = self.display_item_sizes(display_base_width);

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
                                        div()
                                            .relative()
                                            .size_full()
                                            .child(
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
                                            .child(
                                                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(
                                                Scrollbar::vertical(&self.thumbnail_scroll)
                                                    .scrollbar_show(ScrollbarShow::Always),
                    ),
                                            )
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
                                                div()
                                                    .relative()
                                                    .size_full()
                                                    .child(
                                                        v_virtual_list(
                                                            cx.entity(),
                                                            "display-virtual-list",
                                                            display_sizes.clone(),
                                                            move |viewer, visible_range, _window, cx| {
                                                                let target_width =
                                                                    viewer.display_target_width(_window);
                                                                viewer.request_display_load_for_visible_range(
                                                                    visible_range.clone(),
                                                                    target_width,
                                                                    cx,
                                                                );
                                                                visible_range
                                                                    .map(|ix| {
                                                                        let Some(page) = viewer.pages.get(ix)
                                                                        else {
                                                                            return div().into_any_element();
                                                                        };
                                                                        let display_base_width =
                                                                            viewer.display_base_width(_window);
                                                                        let (_, display_height) =
                                                                            viewer.display_card_size(page, display_base_width);
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
                                                                                        page.display_image
                                                                                            .clone(),
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
                                                                                    .when(
                                                                                        page.display_image
                                                                                            .is_none(),
                                                                                        |this| {
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
                                                                                                    .when(
                                                                                                        page.display_failed,
                                                                                                        |this| {
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
                                                                                                        },
                                                                                                    )
                                                                                                    .when(
                                                                                                        !page.display_failed,
                                                                                                        |this| {
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
                                                                                                        },
                                                                                                    ),
                                                                                            )
                                                                                        },
                                                                                    )
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
                                                                                                    .opacity(
                                                                                                        0.9,
                                                                                                    ),
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
                    .bottom_0().child(
                                                        Scrollbar::vertical(&self.display_scroll)
                                                            .scrollbar_show(ScrollbarShow::Always),
                                                            ),
                                                    ),
                                            )
                                        }),
                                ),
                        ),
                ),
        )
    }
}
