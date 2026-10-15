use gpui_kit::*;

#[derive(IntoElement)]
pub struct BlueprintCard {}

impl RenderOnce for BlueprintCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
    }
}
