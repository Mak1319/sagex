//! Kind box — manual `gpui-base` expansion only.
//!
//! Tinted square leading box for a [`FileKind`]: glyph centered on the
//! kind tint. An optional tag strip (e.g. `PNG`) overlays the bottom for
//! thumbnail-style kinds like images.

use gpui_kit::*;

use crate::components::cards::kind::FileKind;
use crate::theme;

#[derive(IntoElement)]
pub struct KindBox {
    kind: FileKind,
    size: f32,
    tag: Option<SharedString>,
}

impl KindBox {
    pub fn new(kind: FileKind) -> Self {
        Self {
            kind,
            size: 32.0,
            tag: None,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn tag(mut self, tag: impl Into<SharedString>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

impl RenderOnce for KindBox {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg, border, fg) = self.kind.tint();
        let edge = px(self.size);
        let mut root = div()
            .relative()
            .w(edge)
            .h(edge)
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .bg(theme::rgb(bg))
            .border_1()
            .border_color(theme::rgb(border))
            .rounded(px(2.0))
            .overflow_hidden()
            .child(
                svg()
                    .path(self.kind.glyph())
                    .w(px(15.0))
                    .h(px(15.0))
                    .text_color(theme::rgb(fg)),
            );
        if let Some(tag) = self.tag {
            root = root.child(
                div()
                    .absolute()
                    .bottom(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .flex()
                    .flex_row()
                    .justify_center()
                    .bg(theme::rgb(fg))
                    .text_size(px(7.0))
                    .text_color(theme::rgb(0xffffff))
                    .child(tag),
            );
        }
        root
    }
}
