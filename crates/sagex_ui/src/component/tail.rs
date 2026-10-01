//! Bubble tail nub as a raw SVG element (not an `Icon`).

use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window, div, px,
    svg,
};

/// 9×12 tail, top edge flush with the bubble top. Side picks the file;
/// `currentColor` fill takes the bubble background via `text_color`.
#[derive(IntoElement)]
pub struct Tail {
    style: StyleRefinement,
    mine: bool,
    color: Hsla,
}

impl Tail {
    pub fn new(mine: bool, color: impl Into<Hsla>) -> Self {
        Self {
            style: StyleRefinement::default(),
            mine,
            color: color.into(),
        }
    }
}

impl Styled for Tail {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Tail {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().child(
            svg()
                .path(if self.mine {
                    "icons/tail-out.svg"
                } else {
                    "icons/tail-in.svg"
                })
                .w(px(9.))
                .h(px(12.))
                .flex_shrink_0()
                .text_color(self.color),
        )
    }
}
