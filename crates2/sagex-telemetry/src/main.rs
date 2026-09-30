mod api;
mod home;
mod keys;
mod model;
mod net;
mod secrets;
mod ui;
mod verify;
mod views;

use std::process::ExitCode;

use gpui::{App, Bounds, WindowBounds, WindowOptions, prelude::*, size};
use gpui::px;

fn main() -> ExitCode {
    let home = match home::resolve() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("sagex-telemetry: home init failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    // SAGEX_TELEMETRY_URL still wins over the config file.
    let gateway_url =
        std::env::var("SAGEX_TELEMETRY_URL").unwrap_or(home.config.gateway_url.clone());
    if !crate::secrets::available() {
        eprintln!("sagex-telemetry: no OS keyring — login keys stay memory-only");
    }

    gpui_platform::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        let bounds = Bounds::centered(None, size(px(1100.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| model::Telemetry::new(window, cx, home, gateway_url)),
        )
        .unwrap();
        cx.activate(true);
    });
    ExitCode::SUCCESS
}
