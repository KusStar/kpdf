use super::display_file_name;
use super::PdfViewer;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::input::Input;
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use gpui_component::*;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone)]
enum CommandPanelItem {
    OpenFile,
    OpenTab {
        tab_id: usize,
        path: PathBuf,
        is_active: bool,
    },
    RecentFile {
        path: PathBuf,
        last_seen_page: Option<usize>,
    },
}

const COMMAND_PANEL_WIDTH: f32 = 560.0;
const COMMAND_PANEL_MAX_HEIGHT: f32 = 460.0;
const COMMAND_PANEL_SCROLLBAR_GUTTER: f32 = 20.0;

impl PdfViewer {
    pub(super) fn open_command_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        if !self.command_panel_open {
            self.command_panel_open = true;
            self.command_panel_needs_focus = true;
            self.needs_root_refocus = false;
            self.command_panel_selected_index = 0;
            self.command_panel_list_scroll.scroll_to_item(0);
            changed = true;
        }
        if self.recent_popup_open {
            self.close_recent_popup(cx);
            changed = true;
        }
        self.command_panel_input_state.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        if changed {
            cx.notify();
        }
    }

    pub(super) fn close_command_panel(&mut self, cx: &mut Context<Self>) {
        if self.command_panel_open {
            self.command_panel_open = false;
            self.command_panel_needs_focus = false;
            self.needs_root_refocus = true;
            cx.notify();
        }
    }

    pub(super) fn toggle_command_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_panel_open {
            self.close_command_panel(cx);
        } else {
            self.open_command_panel(window, cx);
        }
    }

    fn opened_file_tabs(&self) -> Vec<(usize, PathBuf)> {
        self.tab_bar
            .tabs()
            .iter()
            .filter_map(|tab| tab.path.as_ref().map(|path| (tab.id, path.clone())))
            .collect()
    }

    fn command_panel_items(&self) -> Vec<CommandPanelItem> {
        let query = self.command_panel_query.trim().to_ascii_lowercase();
        let query_matches = |path: &PathBuf| {
            if query.is_empty() {
                return true;
            }
            let file_name = display_file_name(path).to_ascii_lowercase();
            let path_text = path.display().to_string().to_ascii_lowercase();
            file_name.contains(&query) || path_text.contains(&query)
        };

        let mut items = vec![CommandPanelItem::OpenFile];
        let active_tab_id = self.tab_bar.active_tab_id();
        let open_files = self.opened_file_tabs();
        let open_file_paths: HashSet<PathBuf> =
            open_files.iter().map(|(_, path)| path.clone()).collect();

        items.extend(open_files.into_iter().filter_map(|(tab_id, path)| {
            if query_matches(&path) {
                Some(CommandPanelItem::OpenTab {
                    tab_id,
                    path,
                    is_active: active_tab_id == Some(tab_id),
                })
            } else {
                None
            }
        }));

        items.extend(
            self.recent_files_with_positions(&self.recent_files)
                .into_iter()
                .filter_map(|(path, last_seen_page)| {
                    if open_file_paths.contains(&path) || !query_matches(&path) {
                        return None;
                    }
                    Some(CommandPanelItem::RecentFile {
                        path,
                        last_seen_page,
                    })
                }),
        );

        items
    }

    pub(super) fn move_command_panel_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let items_len = self.command_panel_items().len();
        if items_len == 0 {
            return;
        }
        let current = self
            .command_panel_selected_index
            .min(items_len.saturating_sub(1)) as isize;
        let next = (current + delta).rem_euclid(items_len as isize) as usize;
        if next != self.command_panel_selected_index {
            self.command_panel_selected_index = next;
            self.command_panel_list_scroll.scroll_to_item(next);
            cx.notify();
        }
    }

    pub(super) fn execute_command_panel_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = self.command_panel_items();
        if items.is_empty() {
            return;
        }
        let index = self
            .command_panel_selected_index
            .min(items.len().saturating_sub(1));
        match items[index].clone() {
            CommandPanelItem::OpenFile => {
                self.close_command_panel(cx);
                self.open_pdf_dialog(window, cx);
            }
            CommandPanelItem::OpenTab { tab_id, .. } => {
                self.close_command_panel(cx);
                self.switch_to_tab(tab_id, cx);
            }
            CommandPanelItem::RecentFile { path, .. } => {
                self.close_command_panel(cx);
                self.open_recent_pdf(path, cx);
            }
        }
    }

    pub(super) fn render_command_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.command_panel_open {
            return None;
        }

        let i18n = self.i18n();
        let viewer = cx.entity();
        let items = self.command_panel_items();
        let selected_index = if items.is_empty() {
            0
        } else {
            self.command_panel_selected_index
                .min(items.len().saturating_sub(1))
        };
        let list_scroll = self.command_panel_list_scroll.clone();
        let list_max_height = (COMMAND_PANEL_MAX_HEIGHT - 44.0).max(120.0);
        let list_content = if items.is_empty() {
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(i18n.no_recent_files())
                .into_any_element()
        } else {
            div()
                .relative()
                .w_full()
                .child(
                    div()
                        .id("command-panel-list")
                        .w_full()
                        .h(px(list_max_height))
                        .overflow_y_scroll()
                        .track_scroll(&list_scroll)
                        .pr(px(COMMAND_PANEL_SCROLLBAR_GUTTER))
                        .v_flex()
                        .gap_1()
                        .children(items.iter().enumerate().map(|(index, item)| {
                            let is_selected = selected_index == index;
                            let item_for_click = item.clone();
                            let (title, subtitle, tail, extra) = match item {
                                CommandPanelItem::OpenFile => (
                                    i18n.choose_file_button().to_string(),
                                    i18n.open_pdf_prompt().to_string(),
                                    None,
                                    None,
                                ),
                                CommandPanelItem::OpenTab {
                                    path, is_active, ..
                                } => (
                                    display_file_name(path),
                                    path.display().to_string(),
                                    if *is_active {
                                        Some(i18n.command_panel_current_badge().to_string())
                                    } else {
                                        None
                                    },
                                    None,
                                ),
                                CommandPanelItem::RecentFile {
                                    path,
                                    last_seen_page,
                                } => (
                                    display_file_name(path),
                                    path.display().to_string(),
                                    None,
                                    last_seen_page
                                        .map(|page_index| i18n.last_seen_page(page_index + 1)),
                                ),
                            };

                            div()
                                .id(("command-panel-item", index))
                                .w_full()
                                .rounded_md()
                                .px_2()
                                .py_1()
                                .cursor_pointer()
                                .when(is_selected, |this| {
                                    this.border_1()
                                        .border_color(cx.theme().primary.opacity(0.65))
                                        .bg(cx.theme().secondary.opacity(0.85))
                                })
                                .when(!is_selected, |this| {
                                    this.hover(|this| this.bg(cx.theme().secondary.opacity(0.6)))
                                        .active(|this| this.bg(cx.theme().secondary.opacity(0.9)))
                                })
                                .on_mouse_move(cx.listener(
                                    move |this, _: &MouseMoveEvent, _, cx| {
                                        if this.command_panel_selected_index != index {
                                            this.command_panel_selected_index = index;
                                            cx.notify();
                                        }
                                    },
                                ))
                                .on_click({
                                    let viewer = viewer.clone();
                                    move |_, window, cx| {
                                        let _ = viewer.update(cx, |this, cx| match item_for_click
                                            .clone()
                                        {
                                            CommandPanelItem::OpenFile => {
                                                this.close_command_panel(cx);
                                                this.open_pdf_dialog(window, cx);
                                            }
                                            CommandPanelItem::OpenTab { tab_id, .. } => {
                                                this.close_command_panel(cx);
                                                this.switch_to_tab(tab_id, cx);
                                            }
                                            CommandPanelItem::RecentFile { path, .. } => {
                                                this.close_command_panel(cx);
                                                this.open_recent_pdf(path, cx);
                                            }
                                        });
                                    }
                                })
                                .child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .v_flex()
                                                .flex_1()
                                                .overflow_x_hidden()
                                                .items_start()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .truncate()
                                                        .text_sm()
                                                        .text_color(cx.theme().foreground)
                                                        .child(title),
                                                )
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .truncate()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(subtitle),
                                                )
                                                .when_some(extra, |this, label| {
                                                    this.child(
                                                        div()
                                                            .w_full()
                                                            .truncate()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(label),
                                                    )
                                                }),
                                        )
                                        .when_some(tail, |this, label| {
                                            this.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().primary)
                                                    .child(label),
                                            )
                                        }),
                                )
                                .into_any_element()
                        })),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .child(
                            Scrollbar::vertical(&list_scroll).scrollbar_show(ScrollbarShow::Always),
                        ),
                )
                .into_any_element()
        };

        Some(
            div()
                .id("command-panel-overlay")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .bg(cx.theme().background.opacity(0.45))
                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                    // Prevent wheel events from leaking to the background lists.
                    cx.stop_propagation();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.close_command_panel(cx);
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .v_flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("command-panel")
                                .w(px(COMMAND_PANEL_WIDTH))
                                .h(px(COMMAND_PANEL_MAX_HEIGHT))
                                .v_flex()
                                .gap_2()
                                .popover_style(cx)
                                .p_2()
                                .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                                    cx.stop_propagation();
                                }))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_, _, _, cx| {
                                        cx.stop_propagation();
                                    }),
                                )
                                .child(
                                    Input::new(&self.command_panel_input_state)
                                        .small()
                                        .cleanable(true),
                                )
                                .child(div().h(px(1.)).bg(cx.theme().border))
                                .child(
                                    div()
                                        .id("command-panel-list-wrap")
                                        .w_full()
                                        .h(px(list_max_height))
                                        .on_scroll_wheel(cx.listener(
                                            |_, _: &ScrollWheelEvent, _, cx| {
                                                cx.stop_propagation();
                                            },
                                        ))
                                        .child(list_content),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
