use gpui_kit::*;

#[derive(IntoElement)]
pub struct AppShell {}

impl RenderOnce for AppShell {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
    }
}
