mod app;
mod assets;
mod backend;
mod chat;
mod component;
mod fonts;
mod pages;
// Auth-gated flow parked (chat opens directly for now); kept compiled.
#[allow(dead_code)]
mod shell;

use assets::SvgAssets;
use backend::{ApiClient, BackendConfig, SessionStore};
use chat::{ChatApp, PollInputs};
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, input::InputState};

// NOTE: chat opens directly (demo seeds + silent live upgrade).

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
            let max_age = cfg.session_max_age_days;

            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    // Follow the OS appearance for the initial paint.
                    Theme::sync_system_appearance(Some(window), cx);

                    // Chat states are created up-front (they need a Window).
                    let search = cx.new(|cx| InputState::new(window, cx));
                    let composer = cx.new(|cx| InputState::new(window, cx));
                    let emoji_search =
                        cx.new(|cx| InputState::new(window, cx).placeholder("Search emoji"));
                    let user_search = cx.new(|cx| InputState::new(window, cx));
                    let room_name = cx.new(|cx| InputState::new(window, cx));
                    let profile_name = cx.new(|cx| InputState::new(window, cx));
                    let stage_caption = cx.new(|cx| InputState::new(window, cx));
                    let stage_pass = cx.new(|cx| InputState::new(window, cx));
                    let poll = PollInputs {
                        q: cx.new(|cx| InputState::new(window, cx)),
                        opts: (0..PollInputs::MAX_OPTS)
                            .map(|_| cx.new(|cx| InputState::new(window, cx)))
                            .collect(),
                    };

                    // Chat screen opens directly: seeds render instantly,
                    // a persisted session upgrades to live silently.
                    let view = cx.new(|_| {
                        ChatApp::new_demo(
                            search,
                            composer,
                            emoji_search,
                            user_search,
                            room_name,
                            profile_name,
                            stage_caption,
                            stage_pass,
                            poll,
                            api.clone(),
                            store.clone(),
                        )
                    });
                    view.update(cx, |this, cx| this.boot_demo(max_age, cx));

                    // Live-follow OS light/dark while System mode is on.
                    let sub = window.observe_window_appearance({
                        let view = view.clone();
                        move |window, cx| {
                            view.update(cx, |this, cx| {
                                if this.theme_mode.is_system() {
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
