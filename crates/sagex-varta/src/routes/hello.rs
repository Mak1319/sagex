use gpui_kit::*;

pub struct HelloPage {}

impl Render for HelloPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
