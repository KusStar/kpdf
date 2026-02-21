impl PdfViewer {
    fn dialog_titlebar_options() -> TitlebarOptions {
        let mut options = TitlebarOptions {
            appears_transparent: true,
            ..Default::default()
        };
        #[cfg(target_os = "macos")]
        {
            options.traffic_light_position = Some(point(px(9.0), px(9.0)));
        }
        options
    }
}
