use gpui::{Div, div, prelude::*, px};

use crate::{
    app::AppState,
    components::{AttachmentRow, Avatar, IconKind, MetaCell, PresenceDot, ThreadRow, badge_el},
    models::Presence,
    theme::palette as p,
};

/// Right inspector: profile + metadata + participants + threads + telemetry.
pub fn inspector(s: &AppState) -> Div {
    let name = s
        .active_channel()
        .map(|c| c.name.to_string())
        .unwrap_or_default();
    let open = s.inspector_open;
    if !open {
        return div();
    }
    let parts = crate::data::participants();
    let online_n = parts
        .iter()
        .filter(|x| x.status == Presence::Online)
        .count();
    let mut col = div()
        .flex()
        .flex_col()
        .w(px(320.0))
        .h_full()
        .bg(p::panel())
        .border_l_1()
        .border_color(p::hairline())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .h(px(56.0))
                .border_b_1()
                .border_color(p::hairline())
                .text_xs()
                .child(div().text_color(p::ink_dim()).child("- CONTEXT / CHANNEL_INSPECT".to_owned()))
                .child(div().text_color(p::ink_faint()).child("›".to_owned())),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(Avatar::new("I").active())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_color(p::ink()).child(name))
                        .child(div().text_xs().text_color(p::ink_dim()).child("# image-processing")),
                ),
        )
        .child(div().text_sm().text_color(p::ink_dim()).child(
            "Discussion about image processing algorithms, blending modes, filters and computer vision.".to_owned(),
        ))
        .child(div().text_xs().text_color(p::ink_faint()).child("CHANNEL METADATA"))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().flex_1().child(MetaCell {
                            key: "CREATED".to_owned(),
                            value: "Aug 12, 2026".to_owned(),
                        }))
                        .child(div().flex_1().child(MetaCell {
                            key: "MEMBERS".to_owned(),
                            value: "5 ACTIVE".to_owned(),
                        })),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().flex_1().child(MetaCell {
                            key: "MESSAGES".to_owned(),
                            value: "342 TOTAL".to_owned(),
                        }))
                        .child(div().flex_1().child(MetaCell {
                            key: "LOG RETENTION".to_owned(),
                            value: "90 DAYS".to_owned(),
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .text_color(p::ink_faint())
                .child(format!("PARTICIPANTS ({})", parts.len()))
                .child(format!("ONLINE: {online_n}/{}", parts.len())),
        );
    for pt in &parts {
        let label = match pt.status {
            Presence::Online if pt.name == "Arjun Mehta" => "OWNER",
            Presence::Online => "ONLINE",
            Presence::Away => "AWAY",
            Presence::Offline => "OFFLINE",
        };
        col = col.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_sm()
                .text_color(p::ink_dim())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(PresenceDot { status: pt.status })
                        .child(pt.name.to_string()),
                )
                .child(badge_el(label)),
        );
    }
    col.child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .pt_2()
            .border_t_1()
            .border_color(p::hairline())
            .child(
                div()
                    .text_xs()
                    .text_color(p::ink_faint())
                    .child("ACTIVE THREADS (2)"),
            )
            .child(ThreadRow {
                title: "Blend mode discussion".to_owned(),
                meta: "4 replies · 2h ago".to_owned(),
                count: Some(3),
            })
            .child(ThreadRow {
                title: "Grayscale support".to_owned(),
                meta: "1 reply · 1d ago".to_owned(),
                count: None,
            })
            .child(
                div()
                    .text_xs()
                    .text_color(p::ink_faint())
                    .child("ATTACHMENTS (3) · TOTAL 4.8 MB".to_owned()),
            )
            .child(AttachmentRow {
                name: "blend-comparison.png".to_owned(),
                size: "320 KB".to_owned(),
                kind: IconKind::Image,
            })
            .child(AttachmentRow {
                name: "shader-notes.md".to_owned(),
                size: "14 KB".to_owned(),
                kind: IconKind::File,
            })
            .child(AttachmentRow {
                name: "test-images.zip".to_owned(),
                size: "4.5 MB".to_owned(),
                kind: IconKind::Download,
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .border_1()
                    .border_color(p::hairline())
                    .bg(p::panel())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_xs()
                            .child(
                                div()
                                    .text_color(p::ink_faint())
                                    .child("TELEMETRY_LINK".to_owned()),
                            )
                            .child(div().text_color(p::ok()).child("● Connected".to_owned())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(p::ink_dim())
                            .child("LATENCY 28 ms · VERSION v0.1.0 · SYNC 14:20:31 UTC".to_owned()),
                    ),
            ),
    )
}
