mod app;
mod assets;
mod backend;
mod chat;
mod component;
mod fonts;
mod pages;
mod shell;

use assets::SvgAssets;
use backend::{ApiClient, BackendConfig, SessionStore};
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, input::InputState, input::OtpState};
use shell::{AppShell, ShellInputs};

fn main() {
    Application::new()
        .with_assets(SvgAssets)
        .run(|cx: &mut App| {
            // Must be called before using any gpui-component features.
            gpui_component::init(cx);
            // Color emoji for the picker Grids (best effort, logs when missing).
            fonts::register_emoji_font(cx);

            // Backend wiring: server URL + 30-day session window come from
            // root `.env` (SAGEX_SERVER_URL / SAGEX_SESSION_MAX_AGE_DAYS).
            let cfg = BackendConfig::load();
            let api = ApiClient::new(&cfg.server_url);
            let store = SessionStore::new(true);

            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    // Follow the OS appearance for the initial paint.
                    Theme::sync_system_appearance(Some(window), cx);

                    // All form states are created up-front (they need a Window).
                    let inputs = ShellInputs {
                        login_email: cx.new(|cx| InputState::new(window, cx)),
                        login_password: cx.new(|cx| InputState::new(window, cx)),
                        signup_name: cx.new(|cx| InputState::new(window, cx)),
                        signup_email: cx.new(|cx| InputState::new(window, cx)),
                        signup_password: cx.new(|cx| InputState::new(window, cx)),
                        signup_confirm: cx.new(|cx| InputState::new(window, cx)),
                        forgot_email: cx.new(|cx| InputState::new(window, cx)),
                        otp: cx.new(|cx| OtpState::new(6, window, cx)),
                        reset_password: cx.new(|cx| InputState::new(window, cx)),
                        reset_confirm: cx.new(|cx| InputState::new(window, cx)),
                        search: cx.new(|cx| InputState::new(window, cx)),
                        composer: cx.new(|cx| InputState::new(window, cx)),
                        emoji_search: cx
                            .new(|cx| InputState::new(window, cx).placeholder("Search emoji")),
                        user_search: cx.new(|cx| InputState::new(window, cx)),
                        room_name: cx.new(|cx| InputState::new(window, cx)),
                        profile_name: cx.new(|cx| InputState::new(window, cx)),
                    };
                    // First launch opens auth-gated; a valid persisted JOSE
                    // session jumps straight into chat (see shell.rs).
                    let view = cx.new(|cx| {
                        AppShell::new(cfg.clone(), api.clone(), store.clone(), inputs, cx)
                    });

                    // Live-follow OS light/dark while the visible view is in System mode.
                    let sub = window.observe_window_appearance({
                        let view = view.clone();
                        move |window, cx| {
                            view.update(cx, |shell, cx| {
                                if shell.follows_system(cx) {
                                    Theme::sync_system_appearance(Some(window), cx);
                                }
                            });
                        }
                    });
                    view.update(cx, |this, _| {
                        this.appearance_sub = Some(sub);
                    });

                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
