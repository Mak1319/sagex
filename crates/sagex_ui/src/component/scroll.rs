//! Custom thin scrollbar thumb: 3px, 0.3 alpha, overflow-only.
//!
//! The 10px gutter is reserved via padding by the parent, so layout never
//! jerks when the thumb appears. Native offset clamping means nothing
//! scrolls when content fits.

use gpui::{
    App, Hsla, IntoElement, ParentElement, PathBuilder, RenderOnce, ScrollHandle, StyleRefinement,
    Styled, Window, canvas, div, point, px,
};

/// Thumb bound to a persistent [`ScrollHandle`]. Paints nothing without overflow.
#[derive(IntoElement)]
pub struct ScrollThumb {
    style: StyleRefinement,
    handle: ScrollHandle,
    color: Hsla,
}

impl ScrollThumb {
    pub fn new(handle: &ScrollHandle, color: impl Into<Hsla>) -> Self {
        Self {
            style: StyleRefinement::default(),
            handle: handle.clone(),
            color: color.into(),
        }
    }
}

impl Styled for ScrollThumb {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ScrollThumb {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let handle = self.handle;
        let color = self.color;
        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(10.))
            .child(
                canvas(
                    move |_, _, _| {},
                    move |bounds, _, window, _| {
                        let vp = f32::from(handle.bounds().size.height);
                        let content =
                            f32::from(handle.max_offset().height + handle.bounds().size.height);
                        if content <= vp + 1.0 {
                            return; // no overflow: invisible, gutter stays
                        }
                        let off = f32::from(handle.offset().y);
                        let h = (vp * vp / content).clamp(20.0, vp);
                        let travel = (vp - h).max(0.0);
                        let span = (content - vp).max(1.0);
                        let y0 = f32::from(bounds.origin.y) + off / span * travel;
                        let y1 = y0 + h;
                        let cxm = f32::from(bounds.origin.x) + 5.0;
                        let r = 1.5;
                        let mut b = PathBuilder::fill();
                        b.move_to(point(px(cxm - r), px(y0 + r)));
                        b.line_to(point(px(cxm - r), px(y1 - r)));
                        b.curve_to(point(px(cxm), px(y1)), point(px(cxm - r), px(y1)));
                        b.curve_to(point(px(cxm + r), px(y1 - r)), point(px(cxm + r), px(y1)));
                        b.line_to(point(px(cxm + r), px(y0 + r)));
                        b.curve_to(point(px(cxm), px(y0)), point(px(cxm + r), px(y0)));
                        b.curve_to(point(px(cxm - r), px(y0 + r)), point(px(cxm - r), px(y0)));
                        b.close();
                        if let Ok(p) = b.build() {
                            window.paint_path(p, color);
                        }
                    },
                )
                .size_full(),
            )
    }
}
