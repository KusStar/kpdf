fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico");
        res.compile()
            .expect("failed to compile Windows icon resource from assets/app.ico");
    }
}
