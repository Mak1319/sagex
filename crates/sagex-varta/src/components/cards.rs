use gpui::{App, IntoElement, RenderOnce, Window, div, prelude::*};

use crate::{
    components::{Icon, IconKind, icon_el},
    theme::palette as p,
};

/// Framed code block with header strip (file, lang, copy).
#[derive(IntoElement)]
pub struct CodeBlock {
    pub file: String,
    pub lang: String,
    pub code: String,
}

impl RenderOnce for CodeBlock {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(p::hairline())
            .bg(p::panel())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(p::hairline())
                    .text_xs()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(icon_el(IconKind::Code, 14.0, false, false))
                            .child(div().text_color(p::ink()).child(self.file))
                            .child(div().text_color(p::ink_faint()).child(self.lang))
                            .child(div().text_color(p::ink_faint()).child("(SHA-256: 8f4a...e12)".to_owned())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_color(p::ink_dim())
                            .child(icon_el(IconKind::Copy, 13.0, false, false))
                            .child("COPY".to_owned()),
                    ),
            )
            .child(div().p_2().text_xs().text_color(p::ink()).child(self.code))
    }
}

/// Schematic figure card (FIG 2.1 style layer-blend pipeline).
#[derive(IntoElement)]
pub struct SchematicCard {
    pub title: String,
    pub lines: Vec<String>,
}

impl RenderOnce for SchematicCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Resting-state FIG card: header + 3-col pipeline grid + channel matrix strip.
        let grid = div()
            .flex()
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .p_1()
                    .border_1()
                    .border_color(p::hairline())
                    .child(div().text_xs().text_color(p::ink_faint()).child("L0 : SOURCE [BASE]".to_owned()))
                    .child(div().text_xs().text_color(p::ink()).child("SRC_RGBA (8-bit)".to_owned()))
                    .child(div().text_xs().text_color(p::ink_faint()).child("Alpha = 1.0".to_owned())),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .p_1()
                    .child(div().text_xs().text_color(p::primary()).child("MULTIPLY (x)".to_owned()))
                    .child(div().text_xs().text_color(p::ink_faint()).child("Norm clamped [0..1]".to_owned())),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .p_1()
                    .border_1()
                    .border_color(p::hairline())
                    .child(div().text_xs().text_color(p::ok()).child("L1 : BLEND TARGET".to_owned()))
                    .child(div().text_xs().text_color(p::ok()).child("BLEND_BUFFER".to_owned()))
                    .child(div().text_xs().text_color(p::ink_faint()).child("Factor = 0.854".to_owned())),
            );
        let mut el = div()
            .flex()
            .flex_col()
            .gap_1()
            .border_1()
            .border_color(p::hairline())
            .bg(p::panel())
            .p_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_color(p::ink_dim())
                            .child(Icon::new(IconKind::Thread).size(14.0).active())
                            .child(self.title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(p::ink_dim())
                            .child("COLOR-SPACE: LINEAR sRGB".to_owned()),
                    ),
            )
            .child(grid);
        for line in self.lines {
            el = el.child(div().text_xs().text_color(p::ink()).child(line));
        }
        el.child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .border_t_1()
                .border_color(p::hairline())
                .pt_1()
                .child(div().text_color(p::primary()).child("CH: RED 0.92".to_owned()))
                .child(div().text_color(p::ok()).child("CH: GRN 0.44".to_owned()))
                .child(div().text_color(p::ink()).child("CH: BLU 0.78".to_owned()))
                .child(div().text_color(p::warn()).child("RES: PASS-01".to_owned())),
        )
    }
}

/// Attachment file card inside a message.
#[derive(IntoElement)]
pub struct AttachmentCard {
    pub name: String,
    pub meta: String,
}

impl RenderOnce for AttachmentCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .border_1()
            .border_color(p::hairline())
            .bg(p::panel())
            .p_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(IconKind::Image).active())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_sm().text_color(p::ink()).child(self.name))
                            .child(div().text_xs().text_color(p::ink_faint()).child(self.meta)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(Icon::new(IconKind::Download).size(16.0))
                    .child(Icon::new(IconKind::More).size(16.0)),
            )
    }
}
