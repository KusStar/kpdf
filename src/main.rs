use gpui::*;
use gpui_component::{button::*, *};
use gpui_component_assets::Assets;

pub struct HelloWorld;
impl Render for HelloWorld {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(cx.theme().font_size);

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
                                .window_control_area(WindowControlArea::Drag)
                                .child("KPDF"),
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
                        .v_flex()
                        .gap_2()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .child("Hello, World!")
                        .child(
                            Button::new("ok")
                                .primary()
                                .label("Let's Go!")
                                .on_click(|_, _, _| println!("Clicked!")),
                        ),
                ),
        )
    }
}

fn main() {
    let app = Application::new().with_assets(Assets);

    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_component::init(cx);
        Theme::change(cx.window_appearance(), None, cx);

        cx.spawn(async move |cx| {
            let window_options = WindowOptions {
                titlebar: Some(TitleBar::title_bar_options()),
                window_decorations: Some(WindowDecorations::Client),
                ..WindowOptions::default()
            };

            cx.open_window(window_options, |_, cx| cx.new(|_| HelloWorld))?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
