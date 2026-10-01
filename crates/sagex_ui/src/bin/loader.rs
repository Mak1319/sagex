//! `sagex_loader` — standalone wizard splash demo (no CLI, no drivers).
//!
//! Animation is configured through code: edit [`SplashConfig`] in
//! `splash.rs` (duration, end behavior, hold, bar color, tagline).

#[path = "loader/splash.rs"]
mod splash;

use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use splash::{SplashAssets, SplashConfig, SplashEnd, SplashView};

fn main() {
    // ---- demo configuration (edit in code) ----
    let cfg = SplashConfig {
        end: SplashEnd::Hold,
        ..Default::default()
    };

    Application::new()
        .with_assets(SplashAssets)
        .run(move |cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1000.0), px(560.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    let view = cx.new(|_| SplashView::new(cfg.clone()));
                    SplashView::boot(&view, cx);
                    view
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
