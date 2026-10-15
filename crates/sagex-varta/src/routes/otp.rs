use gpui_kit::*;

pub struct OtpPage {}

impl Render for OtpPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
