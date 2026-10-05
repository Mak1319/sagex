mod app;

use app::WorkspaceView;
use gpui::{Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;

fn main() {
    let platform = gpui_linux::current_platform(false);
    gpui::Application::with_platform(platform).run(|cx| {
        gpui_component::init(cx);
        let bounds = Bounds::centered(None, size(px(1440.), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| WorkspaceView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();
    });
}
