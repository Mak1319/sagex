use gpui::{
    App, Application, Bounds, ClickEvent, Context, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

struct HelloWorld {
    count: usize,
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child("Hello, World!".to_string())
            .child(format!("Clicks: {}", self.count))
            .child(
                div()
                    .id("counter-button")
                    .flex()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x2e7d32))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0x388e3c)))
                    .active(|style| style.bg(rgb(0x1b5e20)))
                    .child("Click me!")
                    .on_click(cx.listener(
                        |this, _event: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>| {
                            this.count += 1;
                            cx.notify();
                        },
                    )),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.0), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| HelloWorld { count: 0 })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
