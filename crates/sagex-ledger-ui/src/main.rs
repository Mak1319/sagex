mod app;
mod backend;
mod net;
mod runtime;

use app::{tick, Explorer};
use backend::GatewayClient;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, input::InputState};

// Read-only ledger explorer. Talks only to the register gateway over HTTP.
// Records/status/outbox are open endpoints; server logs need the CA-issued
// auditor permit: mint with
//   sagex-certauth issue-permit --identity ledger-auditor [--ttl-hours N]
// and paste it on the Logs tab (or export SAGEX_AUDIT_PERMIT).

fn main() {
    dotenvy::dotenv().ok();
    let base_url = std::env::var("SAGEX_GATEWAY_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());

    Application::new().run(move |cx: &mut App| {
        gpui_component::init(cx);

        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                Theme::sync_system_appearance(Some(window), cx);

                let url_input = cx.new(|cx| InputState::new(window, cx));
                let permit_input = cx.new(|cx| InputState::new(window, cx));
                let wm_input = cx.new(|cx| InputState::new(window, cx));
                let user_input = cx.new(|cx| InputState::new(window, cx));

                let client = GatewayClient::new(&base_url);
                let view = cx.new(|cx| {
                    Explorer::new(
                        client,
                        base_url.clone(),
                        url_input,
                        permit_input,
                        wm_input,
                        user_input,
                        window,
                        cx,
                    )
                });
                view.update(cx, |this, cx| this.boot(cx));
                view.update(cx, |_, cx| tick(cx));

                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
