//! Workspace root view: strict 4-column grid.
//!
//! `50px rail | >=300px index | 1fr stream | >=300px inspector`.
//! Column widths live here as retained state; panels notify on change and
//! GPUI re-renders only affected views (render-on-change, not per-frame).
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::ActiveTheme;

pub struct WorkspaceView {
    pub left_w: f32,
    pub right_w: f32,
}

impl WorkspaceView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { left_w: 300.0, right_w: 300.0 }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(theme.background)
            // 1. icon rail, fixed 50px
            .child(div().w(px(50.)).h_full().bg(theme.sidebar))
            // 2. left index, min 300
            .child(
                div()
                    .w(px(self.left_w))
                    .min_w(px(300.))
                    .h_full()
                    .bg(theme.title_bar),
            )
            // 3. center stream, 1fr
            .child(div().flex_1().h_full().bg(theme.background))
            // 4. right inspector, min 300
            .child(
                div()
                    .w(px(self.right_w))
                    .min_w(px(300.))
                    .h_full()
                    .bg(theme.sidebar),
            )
    }
}
