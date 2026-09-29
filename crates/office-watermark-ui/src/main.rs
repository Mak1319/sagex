mod app;
mod ops;

use app::WatermarkApp;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, input::InputState};

// Desktop GUI for office-watermark: encode / decode / verify sagex:wm
// watermarks in Office files, plus a roundtrip self-test. Same purpose as
// the office-watermark-demo CLI, as a native app.

fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);

        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                Theme::sync_system_appearance(Some(window), cx);

                let text_input = cx.new(|cx| InputState::new(window, cx));
                let output_input = cx.new(|cx| InputState::new(window, cx));
                let rt_text_input = cx.new(|cx| InputState::new(window, cx));

                let view = cx.new(|cx| {
                    WatermarkApp::new(text_input, output_input, rt_text_input, window, cx)
                });

                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
