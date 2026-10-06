use gpui_kit::{prelude::*, *};

use crate::theme::{self, TintColor};

/// 28px letter tile used for chat rows, headers and messages.
#[derive(IntoElement, Clone)]
pub struct NodeAvatar {
    letter: SharedString,
    tint: TintColor,
    presence: bool,
}

impl NodeAvatar {
    pub fn new(letter: impl Into<SharedString>, tint: TintColor) -> Self {
        Self {
            letter: letter.into(),
            tint,
            presence: false,
        }
    }

    /// Green online dot pinned to the tile's bottom-right corner.
    pub fn presence(mut self) -> Self {
        self.presence = true;
        self
    }
}

impl RenderOnce for NodeAvatar {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        let tint = t.tint(self.tint);
        div()
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .w(px(28.0))
            .h(px(28.0))
            .rounded(px(t.radius))
            .bg(tint.bg)
            .text_color(tint.fg)
            .border_1()
            .border_color(tint.border)
            .text_size(px(11.0))
            .font_weight(FontWeight::BOLD)
            .child(self.letter)
            .when(self.presence, |this| {
                this.child(
                    div()
                        .absolute()
                        .bottom(px(-2.0))
                        .right(px(-2.0))
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(t.success)
                        .border_1()
                        .border_color(t.lowest),
                )
            })
    }
}
