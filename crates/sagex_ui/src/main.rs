mod app;
mod assets;
mod chat;
mod component;
mod fonts;
mod pages;

use assets::SvgAssets;
use chat::ChatApp;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, input::InputState};

fn main() {
    Application::new()
        .with_assets(SvgAssets)
        .run(|cx: &mut App| {
            // Must be called before using any gpui-component features.
            gpui_component::init(cx);
            // Color emoji for the picker Grids (best effort, logs when missing).
            fonts::register_emoji_font(cx);

            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    // Follow the OS appearance for the initial paint.
                    Theme::sync_system_appearance(Some(window), cx);

                    // Chat launches on start (auth kept separate in `app`/`pages`).
                    let search = cx.new(|cx| InputState::new(window, cx));
                    let composer = cx.new(|cx| InputState::new(window, cx));
                    let emoji_search =
                        cx.new(|cx| InputState::new(window, cx).placeholder("Search emoji"));
                    let view = cx.new(|_| ChatApp::new(search, composer, emoji_search));

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
