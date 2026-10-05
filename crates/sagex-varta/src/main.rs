mod app;
mod components;
mod data;
mod fonts;
mod icons;
mod panels;
mod svg_loader;
mod theme;

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1440.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "sagex-varta // 804-CHAT",
        opts,
        Box::new(|_cc| Ok(Box::new(app::VartaApp::new()))),
    )
}
