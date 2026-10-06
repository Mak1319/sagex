use gpui_kit::{prelude::*, *};

use crate::{
    components::shared::{IconBtn, NodeAvatar, PresenceDot},
    theme::{self, TintColor},
};

/// Canvas top: channel identity + viewport chip + action buttons.
/// `h_auto` — natural height, never starved by the stream below.
#[derive(IntoElement, Clone)]
pub struct ChatHeader;

impl RenderOnce for ChatHeader {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        div()
            .flex_none()
            .flex()
            .flex_col()
            .w_full()
            .bg(t.lowest)
            .border_b_1()
            .border_color(t.line)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(12.0))
                    .h(px(48.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .min_w_0()
                            .child(NodeAvatar::new("I", TintColor::Blue).presence())
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_size(px(15.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(t.on_surface)
                                                    .child("Image Processing"),
                                            )
                                            .child(
                                                div()
                                                    .px(px(4.0))
                                                    .bg(t.inset)
                                                    .border_1()
                                                    .border_color(t.line)
                                                    .rounded(px(t.radius_sm))
                                                    .text_size(px(9.0))
                                                    .text_color(t.muted)
                                                    .child("ID: #0x88F"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(8.0))
                                            .text_size(px(9.5))
                                            .child(div().text_color(t.on_variant).child("5 nodes"))
                                            .child(div().text_color(t.line_strong).child("•"))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(t.success)
                                                    .child(PresenceDot::online(cx))
                                                    .child("3 online"),
                                            )
                                            .child(div().text_color(t.line_strong).child("•")),
                                        // i removed this so do not add it again
                                        // .child(
                                        //     div()
                                        //         .font_weight(FontWeight::SEMIBOLD)
                                        //         .text_color(t.primary)
                                        //         .child("TOL: 0.001"),
                                        // ),
                                    ),
                            ),
                    )
                    // i remove this intentionally so do not make anything wrong with portion and i know what to do
                    // and what not to do
                    // .child(
                    //     div()
                    //         .flex_none()
                    //         .flex()
                    //         .flex_row()
                    //         .items_center()
                    //         .gap(px(4.0))
                    //         .px(px(8.0))
                    //         .py(px(2.0))
                    //         .bg(t.inset)
                    //         .border_1()
                    //         .border_color(t.line)
                    //         .rounded(px(t.radius_sm))
                    //         .text_size(px(9.0))
                    //         .text_color(t.muted)
                    //         .child("|← 800px FLUID VIEWPORT →|"),
                    // )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .text_color(t.muted)
                            // Ai instruction: Do not add it these again i removed this intentionally
                            // .child(IconBtn::new("icons/video-camera.svg", 16.0, 28.0))
                            // .child(IconBtn::new("icons/group.svg", 16.0, 28.0))
                            // .child(IconBtn::new("icons/search.svg", 16.0, 28.0))
                            // .child(div().flex_none().w(px(1.0)).h(px(16.0)).bg(t.line))
                            .child(IconBtn::new("icons/more-vert.svg", 16.0, 28.0)),
                    ),
            )
            // Axis ruler approximation: hairline under the header.
            .child(div().flex_none().w_full().h(px(1.0)).bg(t.line))
    }
}
