mod pdf_viewer;

use gpui::*;
use gpui_component::*;
use gpui_component_assets::Assets;
use pdf_viewer::PdfViewer;

fn main() {
    let app = Application::new().with_assets(Assets);

    app.run(move |cx| {
        gpui_component::init(cx);
        Theme::change(cx.window_appearance(), None, cx);

        cx.spawn(async move |cx| {
            let window_options = WindowOptions {
                titlebar: Some(TitleBar::title_bar_options()),
                window_decorations: Some(WindowDecorations::Client),
                ..WindowOptions::default()
            };

            cx.open_window(window_options, |_, cx| cx.new(|_| PdfViewer::new()))?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
