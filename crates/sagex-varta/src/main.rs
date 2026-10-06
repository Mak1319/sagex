mod app;
mod assets;
mod components;
mod theme;

use app::App;
use assets::LocalAssets;
use gpui_kit::component::*;
use gpui_kit::*;

fn main() {
    gpui_kit::application()
        .with_assets(LocalAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::init(cx);
            let bounds = Bounds::centered(None, size(px(1000.0), px(700.0)), cx);
            let mut opts = TitleBar::window_options();
            opts.window_bounds = Some(WindowBounds::Windowed(bounds));
            // Linux: client decorations removes the server-side system bar in the screenshot.
            // macOS/Windows: appears_transparent (inside window_options) hides native chrome.
            opts.window_decorations = Some(WindowDecorations::Client);
            cx.spawn(async move |cx| {
                cx.open_window(opts, |window, cx| {
                    let view = cx.new(|cx| App::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
}
