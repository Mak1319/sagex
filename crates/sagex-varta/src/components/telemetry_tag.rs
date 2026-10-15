use gpui_kit::*;

#[derive(IntoElement)]
pub struct TelemetryTag {}

impl RenderOnce for TelemetryTag {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
    }
}
