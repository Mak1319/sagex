use gpui_kit::*;

pub struct SetupPage {}

impl Render for SetupPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
