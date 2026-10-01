mod api;
mod crypto;
mod home;
mod model;
mod net;
mod secrets;
mod ui;
mod views;

use std::process::ExitCode;

use gpui::{App, Bounds, WindowBounds, WindowOptions, prelude::*, size};
use gpui::px;

fn main() -> ExitCode {
    let home = match home::resolve() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("sagex-desk: home init failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    // SAGEX_BASE still wins over the config file.
    let base_url = std::env::var("SAGEX_BASE").unwrap_or(home.config.server_url.clone());
    if !crate::secrets::available() {
        eprintln!("sagex-desk: no OS keyring — refresh tokens stay memory-only");
    }

    gpui_platform::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        let bounds = Bounds::centered(None, size(px(1100.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| model::Desk::new(window, cx, home, base_url)),
        )
        .unwrap();
        cx.activate(true);
    });
    ExitCode::SUCCESS
}
