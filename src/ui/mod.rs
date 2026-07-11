#[cfg(feature = "ui")]
mod app;

#[cfg(feature = "ui")]
pub fn run() {
    use eframe::NativeOptions;
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FNVIB — FNV Interior Builder")
            .with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };
    eframe::run_native(
        "fnvib",
        options,
        Box::new(|cc| Ok(Box::new(app::FnvibApp::new(cc)))),
    )
    .unwrap_or_else(|e| eprintln!("UI error: {e}"));
}
