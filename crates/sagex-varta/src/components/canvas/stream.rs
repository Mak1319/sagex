use gpui_kit::{prelude::*, *};

use crate::{
    components::{
        canvas::message::{FileData, MessageCard, MessageData},
        shared::PresenceDot,
    },
    theme::{self, TintColor},
};

/// Static stream content (mock data until send exists).
const MESSAGES: [MessageData; 2] = [
    MessageData {
        avatar: 'A',
        tint: TintColor::Emerald,
        name: "Arjun Mehta",
        name_primary: false,
        time: "14:15:32 UTC",
        chip: Some(("OWNER", TintColor::Blue)),
        loc: "LOC: S1-02",
        body_md: "Nice! Can you share the updated shader code? Also, does this work with grayscale images or do we require an additional channel expansion step?",
        file: None,
    },
    MessageData {
        avatar: 'P',
        tint: TintColor::Red,
        name: "Priya Sharma",
        name_primary: false,
        time: "14:20:02 UTC",
        chip: Some(("CORE-ALGO", TintColor::Emerald)),
        loc: "LOC: S1-04",
        body_md: "Here are the test images and the comparison results rendered through the WebGL pipeline:",
        file: Some(FileData {
            name: "blend-comparison.png",
            meta: "PNG • 320 KB • 1024 x 683 px",
            icon: "icons/media-image.svg",
        }),
    },
];

/// Centered timestep divider chip.
#[derive(IntoElement, Clone)]
pub struct TimestampDivider;

impl RenderOnce for TimestampDivider {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        div()
            .flex_none()
            .flex()
            .flex_row()
            .justify_center()
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .py(px(2.0))
                    .bg(t.lowest)
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(t.radius_sm))
                    .text_size(px(9.0))
                    .child(PresenceDot::new(t.primary))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(t.on_variant)
                            .child("TIMESTEP: OCT 04, 2026"),
                    )
                    .child(div().text_color(t.line_strong).child("|"))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.primary)
                            .child("T_ZERO: 14:12 UTC"),
                    ),
            )
    }
}

/// Scrollable message stream: divider + mock messages.
#[derive(IntoElement, Clone)]
pub struct Stream;

impl RenderOnce for Stream {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        div()
            .id("canvas-stream-scroll")
            .flex()
            .flex_1()
            .flex_col()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .bg(t.canvas)
            .relative()
            .child({
                div()
                    .absolute()
                    .size_full()
                    .gap(px(12.0))
                    .py(px(12.0))
                    .px(px(16.0))
            .child(TimestampDivider)
            .children(
                MESSAGES
                    .iter()
                    .enumerate()
                    .map(|(index, m)| MessageCard::new(index, m.clone())),
            )
            })
    }
}
