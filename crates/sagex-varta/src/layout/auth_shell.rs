use gpui_kit::*;

#[derive(IntoElement)]
pub struct AuthShell {}

impl RenderOnce for AuthShell {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
    }
}
