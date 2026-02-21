mod command_panel;
mod display_list;
#[cfg(target_os = "macos")]
mod macos_context_menu;
mod menu_bar;
pub mod tab;
mod text_selection;
mod thumbnail_list;
mod utils;

use crate::i18n::{I18n, Language};
use crate::{
    APP_REPOSITORY_URL, CheckForUpdatesMenu, DisableLoggingMenu, EnableLoggingMenu, OpenLogsMenu,
    ShowAboutMenu, ShowSettingsMenu, configure_app_menus, updater,
};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::popover::{Popover, PopoverState};
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::text::TextView;
use gpui_component::{button::*, *};
#[cfg(target_os = "windows")]
use raw_window_handle::RawWindowHandle;
use std::cell::Cell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use std::time::Duration;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{SW_RESTORE, ShowWindowAsync};

include!("types.rs");
include!("constants.rs");
include!("window_utils.rs");

use self::tab::{PdfTab, TabBar};
use self::text_selection::copy_to_clipboard;
use self::utils::{
    display_file_name, ensure_pdfium_ready, load_display_images, load_document_summary,
};

pub use self::utils::PageSummary;

pub struct PdfViewer {
    focus_handle: FocusHandle,
    language: Language,
    tab_bar: TabBar,
    recent_store: Option<sled::Tree>,
    position_store: Option<sled::Tree>,
    window_size_store: Option<sled::Tree>,
    open_tabs_store: Option<sled::Tree>,
    titlebar_preferences_store: Option<sled::Tree>,
    theme_preferences_store: Option<sled::Tree>,
    bookmarks_store: Option<sled::Tree>,
    notes_store: Option<sled::Tree>,
    last_window_size: Option<(f32, f32)>,
    theme_mode: ThemeMode,
    preferred_light_theme_name: Option<String>,
    preferred_dark_theme_name: Option<String>,
    titlebar_preferences: TitleBarVisibilityPreferences,
    recent_files: Vec<PathBuf>,
    recent_popup_open: bool,
    recent_popup_trigger_hovered: bool,
    recent_popup_tab_trigger_hovered: bool,
    recent_popup_panel_hovered: bool,
    recent_popup_hover_epoch: u64,
    recent_popup_anchor: Option<RecentPopupAnchor>,
    bookmarks: Vec<BookmarkEntry>,
    markdown_notes: Vec<MarkdownNoteEntry>,
    bookmark_popup_open: bool,
    bookmark_scope: BookmarkScope,
    bookmark_popup_trigger_hovered: bool,
    bookmark_popup_panel_hovered: bool,
    bookmark_popup_hover_epoch: u64,
    bookmark_popup_expanded_notes: Option<(PathBuf, usize)>,
    note_editor_open: bool,
    note_editor_anchor: Option<MarkdownNoteAnchor>,
    note_editor_edit_note_id: Option<u64>,
    note_editor_window: Option<AnyWindowHandle>,
    note_editor_session: u64,
    about_dialog_open: bool,
    settings_dialog_open: bool,
    updater_state: UpdaterUiState,
    command_panel_open: bool,
    command_panel_query: String,
    command_panel_selected_index: usize,
    tab_bar_scroll: ScrollHandle,
    recent_popup_list_scroll: ScrollHandle,
    bookmark_popup_list_scroll: ScrollHandle,
    command_panel_list_scroll: ScrollHandle,
    recent_home_list_scroll: ScrollHandle,
    command_panel_input_state: Entity<InputState>,
    _command_panel_input_subscription: Subscription,
    theme_color_select_state: Entity<SelectState<SearchableVec<SharedString>>>,
    _theme_color_select_subscription: Subscription,
    _theme_registry_subscription: Subscription,
    context_menu_open: bool,
    context_menu_position: Option<Point<Pixels>>,
    context_menu_tab_id: Option<usize>,
    context_menu_note_anchor: Option<MarkdownNoteAnchor>,
    context_menu_note_id: Option<u64>,
    hovered_markdown_note_id: Option<u64>,
    hovered_tab_id: Option<usize>,
    // 拖放相关状态
    drag_state: DragState,
    drag_mouse_position: Option<Point<Pixels>>,
    pending_drag_start: Option<(usize, Point<Pixels>)>,
    text_hover_target: Option<(usize, usize)>, // (tab_id, page_index)
    needs_initial_focus: bool,
    command_panel_needs_focus: bool,
    needs_root_refocus: bool,
    resize_restore_epoch: u64,
}

impl PdfViewer {
    fn now_unix_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }

    fn now_unix_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language = Language::detect();
        let (
            recent_store,
            position_store,
            window_size_store,
            open_tabs_store,
            titlebar_preferences_store,
            theme_preferences_store,
            bookmarks_store,
            notes_store,
        ) = Self::open_persistent_stores();
        let recent_files = recent_store
            .as_ref()
            .map(Self::load_recent_files_from_store)
            .unwrap_or_default();
        let (saved_open_tab_paths, saved_active_open_tab_index) = open_tabs_store
            .as_ref()
            .map(Self::load_open_tabs_from_store)
            .unwrap_or_else(|| (Vec::new(), None));
        let titlebar_preferences = titlebar_preferences_store
            .as_ref()
            .map(Self::load_titlebar_preferences_from_store)
            .unwrap_or_default();
        let (theme_mode, preferred_light_theme_name, preferred_dark_theme_name) =
            theme_preferences_store
                .as_ref()
                .map(|store| {
                    Self::load_theme_preferences_from_store(
                        store,
                        ThemeMode::from(window.appearance()),
                    )
                })
                .unwrap_or_else(|| (ThemeMode::from(window.appearance()), None, None));
        let bookmarks = bookmarks_store
            .as_ref()
            .map(Self::load_bookmarks_from_store)
            .unwrap_or_default();
        let markdown_notes = notes_store
            .as_ref()
            .map(Self::load_markdown_notes_from_store)
            .unwrap_or_default();
        let command_panel_input_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(I18n::new(language).command_panel_search_hint)
        });
        let command_panel_input_state_for_sub = command_panel_input_state.clone();
        let command_panel_input_subscription = cx.subscribe(
            &command_panel_input_state_for_sub,
            |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let next_query = input.read(cx).value().to_string();
                if this.command_panel_query != next_query {
                    this.command_panel_query = next_query;
                    this.command_panel_selected_index = 0;
                    this.command_panel_list_scroll.scroll_to_item(0);
                    if this.command_panel_open {
                        cx.notify();
                    }
                }
            },
        );
        let theme_color_select_state = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<SharedString>::new()),
                None,
                window,
                cx,
            )
        });
        let theme_color_select_state_for_sub = theme_color_select_state.clone();
        let theme_color_select_subscription = cx.subscribe(
            &theme_color_select_state_for_sub,
            |this, _, event: &SelectEvent<SearchableVec<SharedString>>, cx| {
                let SelectEvent::Confirm(theme_name) = event;
                let Some(theme_name) = theme_name.as_ref() else {
                    return;
                };
                this.set_theme_color_by_name(this.theme_mode, theme_name.as_ref(), cx);
            },
        );
        let theme_registry_subscription =
            cx.observe_global_in::<ThemeRegistry>(window, |this, window, cx| {
                this.apply_theme_preferences(Some(window), cx);
                this.sync_theme_color_select(window, cx);
                cx.notify();
            });

        let mut tab_bar = TabBar::new();
        let mut tabs_to_restore = Vec::new();
        for path in saved_open_tab_paths {
            if !path.exists() {
                continue;
            }
            let tab_id = tab_bar.create_tab_with_path(path.clone(), Vec::new());
            tabs_to_restore.push((tab_id, path));
        }

        if tabs_to_restore.is_empty() {
            // 没有可恢复标签时，创建第一个空标签页
            tab_bar.create_tab();
        } else {
            let target_active_index = saved_active_open_tab_index
                .unwrap_or_else(|| tabs_to_restore.len().saturating_sub(1))
                .min(tabs_to_restore.len().saturating_sub(1));
            if let Some((tab_id, _)) = tabs_to_restore.get(target_active_index) {
                tab_bar.switch_to_tab(*tab_id);
            }
        }

        let mut viewer = Self {
            focus_handle: cx.focus_handle(),
            language,
            tab_bar,
            recent_store,
            position_store,
            window_size_store,
            open_tabs_store,
            titlebar_preferences_store,
            theme_preferences_store,
            bookmarks_store,
            notes_store,
            last_window_size: None,
            theme_mode,
            preferred_light_theme_name,
            preferred_dark_theme_name,
            titlebar_preferences,
            recent_files,
            recent_popup_open: false,
            recent_popup_trigger_hovered: false,
            recent_popup_tab_trigger_hovered: false,
            recent_popup_panel_hovered: false,
            recent_popup_hover_epoch: 0,
            recent_popup_anchor: None,
            bookmarks,
            markdown_notes,
            bookmark_popup_open: false,
            bookmark_scope: BookmarkScope::CurrentPdf,
            bookmark_popup_trigger_hovered: false,
            bookmark_popup_panel_hovered: false,
            bookmark_popup_hover_epoch: 0,
            bookmark_popup_expanded_notes: None,
            note_editor_open: false,
            note_editor_anchor: None,
            note_editor_edit_note_id: None,
            note_editor_window: None,
            note_editor_session: 0,
            about_dialog_open: false,
            settings_dialog_open: false,
            updater_state: UpdaterUiState::Idle,
            command_panel_open: false,
            command_panel_query: String::new(),
            command_panel_selected_index: 0,
            tab_bar_scroll: ScrollHandle::new(),
            recent_popup_list_scroll: ScrollHandle::new(),
            bookmark_popup_list_scroll: ScrollHandle::new(),
            command_panel_list_scroll: ScrollHandle::new(),
            recent_home_list_scroll: ScrollHandle::new(),
            command_panel_input_state,
            _command_panel_input_subscription: command_panel_input_subscription,
            theme_color_select_state,
            _theme_color_select_subscription: theme_color_select_subscription,
            _theme_registry_subscription: theme_registry_subscription,
            context_menu_open: false,
            context_menu_position: None,
            context_menu_tab_id: None,
            context_menu_note_anchor: None,
            context_menu_note_id: None,
            hovered_markdown_note_id: None,
            hovered_tab_id: None,
            drag_state: DragState::None,
            drag_mouse_position: None,
            pending_drag_start: None,
            text_hover_target: None,
            needs_initial_focus: true,
            command_panel_needs_focus: false,
            needs_root_refocus: false,
            resize_restore_epoch: 0,
        };

        viewer.apply_theme_preferences(Some(window), cx);
        viewer.sync_theme_color_select(window, cx);
        viewer.persist_open_tabs();
        if !tabs_to_restore.is_empty()
            && let Err(err) = ensure_pdfium_ready(language)
        {
            crate::debug_log!("[pdfium] pre-init before restoring tabs failed: {}", err);
        }
        viewer.restore_open_tabs(tabs_to_restore, cx);
        viewer
    }
}

include!("core.rs");
include!("file_actions.rs");
include!("settings_dialogs.rs");
include!("tab_actions.rs");
include!("recent_bookmark_notes.rs");
include!("page_rendering.rs");
include!("interactions.rs");

impl Focusable for PdfViewer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PdfViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_initial_focus {
            self.needs_initial_focus = false;
            cx.focus_self(window);
        }
        if self.command_panel_open && self.command_panel_needs_focus {
            self.command_panel_needs_focus = false;
            let _ = self
                .command_panel_input_state
                .update(cx, |input, cx| input.focus(window, cx));
        }
        if !self.command_panel_open && self.needs_root_refocus {
            self.needs_root_refocus = false;
            window.focus(&self.focus_handle);
        }

        window.set_rem_size(cx.theme().font_size);

        let bounds = window.bounds();
        let current_size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let mut window_size_changed = false;
        if self.last_window_size != Some(current_size) {
            self.last_window_size = Some(current_size);
            if !window.is_maximized() && !window.is_fullscreen() {
                self.save_window_size(current_size.0, current_size.1);
            }
            window_size_changed = true;
        }

        let (
            page_count,
            current_page_num,
            zoom,
            _file_name,
            thumbnail_sizes,
            display_sizes,
            _display_base_width,
            display_panel_width,
        ) = {
            let active_tab = self.active_tab();
            let page_count = active_tab.map(|t| t.pages.len()).unwrap_or(0);
            let current_page_num = if page_count == 0 {
                0
            } else {
                active_tab.map(|t| t.active_page + 1).unwrap_or(0)
            };
            let zoom = active_tab.map(|t| t.zoom).unwrap_or(1.0);

            let file_name = active_tab
                .and_then(|t| t.path.as_ref())
                .map(|p| display_file_name(p))
                .unwrap_or_else(|| self.i18n().file_not_opened.to_string());

            let display_base_width = active_tab
                .map(|t| self.display_base_width(window, t.zoom))
                .unwrap_or(DISPLAY_MIN_WIDTH);
            let display_panel_width = active_tab
                .map(|t| self.display_panel_width(window, t.zoom))
                .unwrap_or(DISPLAY_MIN_WIDTH);

            let thumbnail_sizes = active_tab
                .map(|t| self.thumbnail_item_sizes(&t.pages))
                .unwrap_or_else(|| Rc::new(Vec::new()));
            let display_sizes = active_tab
                .map(|t| self.display_item_sizes(&t.pages, display_base_width))
                .unwrap_or_else(|| Rc::new(Vec::new()));

            (
                page_count,
                current_page_num,
                zoom,
                file_name,
                thumbnail_sizes,
                display_sizes,
                display_base_width,
                display_panel_width,
            )
        };

        let zoom_label: SharedString = format!("{:.0}%", zoom * 100.0).into();

        // 更新当前标签页的显示滚动偏移
        let target_width = if let Some(tab) = self.active_tab() {
            self.display_target_width(window, tab.zoom)
        } else {
            220
        };
        let mut display_layout_changed = false;
        let mut page_to_restore_after_layout_change = None;
        if let Some(tab) = self.active_tab_mut() {
            let target_width_changed = tab.last_display_target_width != target_width;
            display_layout_changed = window_size_changed || target_width_changed;

            if display_layout_changed {
                // Invalidate pending scroll-to-page sync jobs from previous layout.
                tab.display_scroll_sync_epoch = tab.display_scroll_sync_epoch.wrapping_add(1);
            }

            if target_width_changed {
                tab.last_display_target_width = target_width;
            }

            if display_layout_changed {
                if tab.pages.is_empty() {
                    tab.last_display_scroll_offset = Some(tab.display_scroll.offset());
                } else {
                    // Keep the current page stable across resize/maximize/restore.
                    let keep_page = tab.active_page.min(tab.pages.len().saturating_sub(1));
                    tab.active_page = keep_page;
                    tab.selected_page = keep_page;
                    tab.last_display_visible_range =
                        Some(keep_page..keep_page.saturating_add(1).min(tab.pages.len()));
                    tab.last_display_scroll_offset = Some(tab.display_scroll.offset());
                    page_to_restore_after_layout_change = Some(keep_page);
                }
            }
        }
        if let Some(keep_page) = page_to_restore_after_layout_change {
            self.schedule_restore_current_page_after_layout_change(keep_page, cx);
        }
        if !display_layout_changed {
            self.on_display_scroll_offset_changed(cx);
        }

        let context_menu = self.render_context_menu(cx);
        let drag_tab_preview = self.render_drag_tab_preview(cx);
        let command_panel = self.render_command_panel(cx);
        let about_dialog = self.render_about_dialog(cx);
        let settings_dialog = self.render_settings_dialog(cx);

        div()
            .size_full()
            .on_action(cx.listener(|this, _: &ShowAboutMenu, _, cx| {
                this.open_about_dialog(cx);
            }))
            .on_action(cx.listener(|this, _: &CheckForUpdatesMenu, _, cx| {
                this.open_about_dialog(cx);
                this.check_for_updates(cx);
            }))
            .on_action(cx.listener(|this, _: &ShowSettingsMenu, window, cx| {
                this.open_settings_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &EnableLoggingMenu, _, cx| {
                if crate::logger::enable_file_logging() {
                    configure_app_menus(cx, this.i18n());
                }
            }))
            .on_action(cx.listener(|this, _: &DisableLoggingMenu, _, cx| {
                crate::logger::disable_file_logging();
                configure_app_menus(cx, this.i18n());
            }))
            .on_action(cx.listener(|this, _: &OpenLogsMenu, _, _| {
                this.open_logs_directory();
            }))
            .child(
                div()
                    .v_flex()
                    .size_full()
                    .bg(cx.theme().background)
                    .relative()
                    .track_focus(&self.focus_handle)
                    .capture_key_down(cx.listener(
                        |this, event: &gpui::KeyDownEvent, window, cx| {
                            this.handle_key_down(event, window, cx);
                        },
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.close_context_menu(cx);
                            this.close_bookmark_popup(cx);
                            window.focus(&this.focus_handle);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_drag_mouse_position(event.position, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.finish_tab_drag(cx);
                        }),
                    )
                    .child(
                        div()
                            .id("title-bar")
                            .w_full()
                            .v_flex()
                            .bg(cx.theme().title_bar)
                            .child(
                                div()
                                    .id("title-bar-top")
                                    .h(px(TITLE_BAR_HEIGHT))
                                    .w_full()
                                    .relative()
                                    .border_b_1()
                                    .border_color(cx.theme().title_bar_border)
                                    .when(cfg!(target_os = "macos"), |this| {
                                        this.child(
                                            div()
                                                .id("title-drag-area")
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .right_0()
                                                .bottom_0()
                                                .on_double_click(|_, window, _| {
                                                    window.titlebar_double_click()
                                                })
                                                .window_control_area(WindowControlArea::Drag),
                                        )
                                    })
                                    .child(
                                        div()
                                            .id("title-bar-foreground")
                                            .h_full()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .id("title-nav-host")
                                                    .h_full()
                                                    .flex_1()
                                                    .pl(px(TITLE_BAR_CONTENT_LEFT_PADDING))
                                                    .pr_1()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(self.render_menu_bar(
                                                        page_count,
                                                        current_page_num,
                                                        zoom_label,
                                                        self.titlebar_preferences.show_navigation,
                                                        self.titlebar_preferences.show_zoom,
                                                        cx,
                                                    )),
                                            )
                                            .when(!cfg!(target_os = "macos"), |this| {
                                                this.child(
                                                    div()
                                                        .id("title-drag-area")
                                                        .h_full()
                                                        .w(px(24.))
                                                        .flex_shrink_0()
                                                        .map(|this| {
                                                            let should_move =
                                                                Rc::new(Cell::new(false));
                                                            this.on_double_click(
                                                                |_, window, _| window.zoom_window(),
                                                            )
                                                            .on_mouse_down(MouseButton::Left, {
                                                                let should_move =
                                                                    should_move.clone();
                                                                move |_, _, _| {
                                                                    should_move.set(true);
                                                                }
                                                            })
                                                            .on_mouse_down_out({
                                                                let should_move =
                                                                    should_move.clone();
                                                                move |_, _, _| {
                                                                    should_move.set(false);
                                                                }
                                                            })
                                                            .on_mouse_up(MouseButton::Left, {
                                                                let should_move =
                                                                    should_move.clone();
                                                                move |_, _, _| {
                                                                    should_move.set(false);
                                                                }
                                                            })
                                                            .on_mouse_move({
                                                                let should_move =
                                                                    should_move.clone();
                                                                move |_, window, _| {
                                                                    if should_move.get() {
                                                                        should_move.set(false);
                                                                        window.start_window_move();
                                                                    }
                                                                }
                                                            })
                                                            .window_control_area(
                                                                WindowControlArea::Drag,
                                                            )
                                                        }),
                                                )
                                            })
                                            .when(!cfg!(target_os = "macos"), |this| {
                                                this.child(
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
                                                                    Icon::new(
                                                                        crate::icons::IconName::WindowMinimize,
                                                                    )
                                                                    .text_color(cx.theme().foreground),
                                                                )
                                                                .on_click(|_, window, _| {
                                                                    window.minimize_window()
                                                                }),
                                                        )
                                                        .child(
                                                            Button::new("window-maximize")
                                                                .ghost()
                                                                .small()
                                                                .icon(
                                                                    Icon::new(if window.is_maximized() {
                                                                        crate::icons::IconName::WindowRestore
                                                                    } else {
                                                                        crate::icons::IconName::WindowMaximize
                                                                    })
                                                                    .text_color(cx.theme().foreground),
                                                                )
                                                                .on_click(|_, window, _| {
                                                                    zoom_or_restore_window(window)
                                                                }),
                                                        )
                                                        .child(
                                                            Button::new("window-close")
                                                                .ghost()
                                                                .small()
                                                                .icon(
                                                                    Icon::new(crate::icons::IconName::WindowClose)
                                                                        .text_color(cx.theme().foreground),
                                                                )
                                                                .on_click(|_, window, _| {
                                                                    window.remove_window()
                                                                }),
                                                        ),
                                                )
                                            }),
                                    )
                            )
                            .child(self.render_tab_bar(cx))
                    )
                    .child(
                        div()
                            .h_full()
                            .w_full()
                            .flex()
                            .overflow_hidden()
                            .when(self.show_thumbnail_panel(), |this| {
                                this.child(self.render_thumbnail_panel(
                                    page_count,
                                    thumbnail_sizes,
                                    cx,
                                ))
                            })
                            .child(self.render_display_panel(
                                page_count,
                                display_sizes,
                                display_panel_width,
                                cx,
                            )),
                    )
                    .when(context_menu.is_some(), |this| {
                        this.child(context_menu.unwrap())
                    })
                    .when(drag_tab_preview.is_some(), |this| {
                        this.child(drag_tab_preview.unwrap())
                    })
                    .when(command_panel.is_some(), |this| {
                        this.child(command_panel.unwrap())
                    })
                    .when(about_dialog.is_some(), |this| {
                        this.child(about_dialog.unwrap())
                    })
                    .when(settings_dialog.is_some(), |this| {
                        this.child(settings_dialog.unwrap())
                    }),
            )
    }
}
