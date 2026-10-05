use gpui::{Context, Div, Entity, div, prelude::*, px};

use crate::{
    Root,
    app::AppState,
    components::{Composer, ContextMenu, IconButton, IconKind, MessageCard},
    models::Route,
    theme::palette as p,
};

/// Center canvas: header + stream + composer.
pub fn chat(root: &Entity<Root>, s: &AppState, cx: &mut Context<Root>) -> Div {
    let ch = s.active_channel();
    let msgs = match s.route {
        Route::Channel(id) => s.feed.get(&id).cloned().unwrap_or_default(),
        _ => vec![],
    };
    let title = ch.map(|c| c.name.to_string()).unwrap_or_default();
    let online = ch.map(|c| c.online).unwrap_or(false);
    let members = "5 members · 3 online".to_string();
    let draft = s.draft.clone();
    let md = s.markdown_tab;
    let insp_open = s.inspector_open;

    let mut stream = div()
        .id("chat-stream")
        .flex()
        .flex_col()
        .flex_1()
        .overflow_y_scroll()
        .p_3()
        .gap_2();
    for m in msgs {
        stream = stream.child(cx.new(|_| MessageCard { message: m }));
    }

    let h_toggle = root.clone();
    let h_tab = root.clone();
    let h_send = root.clone();
    let composer = cx.new(|_| {
        let t = h_tab.clone();
        let se = h_send.clone();
        Composer::new(draft, md)
            .on_tab(move |_, _, cx| {
                let _ = t.update(cx, |this, cx| {
                    this.state.markdown_tab = !this.state.markdown_tab;
                    cx.notify();
                });
            })
            .on_send(move |_, _, cx| {
                let _ = se.update(cx, |this, cx| {
                    this.state.send_draft();
                    cx.notify();
                });
            })
    });

    let status = if online { "● LIVE" } else { "○ IDLE" };
    div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .bg(p::canvas())
        .child(
            div()
                .h(px(56.0))
                .px_4()
                .flex()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(p::hairline())
                .bg(p::panel())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_color(p::ink()).child(title))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .text_xs()
                                .child(div().text_color(p::ink_faint()).child(members))
                                .child(div().text_color(p::ink_dim()).child(status.to_owned())),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(IconButton::new(IconKind::Video))
                        .child(IconButton::new(IconKind::Call))
                        .child(IconButton::new(IconKind::Add))
                        .child(IconButton::new(IconKind::Search))
                        .child(IconButton::new(IconKind::More))
                        .child(
                            div()
                                .id("inspector-toggle")
                                .px_2()
                                .py_1()
                                .text_xs()
                                .border_1()
                                .border_color(p::hairline())
                                .cursor_pointer()
                                .text_color(p::ink_dim())
                                .on_click(move |_, _, cx| {
                                    let _ = h_toggle.update(cx, |this, cx| {
                                        this.state.inspector_open = !this.state.inspector_open;
                                        cx.notify();
                                    });
                                })
                                .child(if insp_open { "HIDE" } else { "SHOW" }.to_owned()),
                        ),
                ),
        )
        .child(stream)
        .child(if s.show_context {
            div()
                .px_3()
                .pb_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(p::ink_faint())
                        .child("M toggles MSG // CONTEXT_OPS · A toggles ACTIONS".to_owned()),
                )
                .child(ContextMenu)
        } else {
            div()
        })
        .child(if s.show_actions {
            div().px_3().pb_2().text_xs().text_color(p::ink_dim()).child(
                "ACTIONS: Inspect Node (⌥I) · Copy Shader Ref (⌘C)".to_owned(),
            )
        } else {
            div()
        })
        .child(composer)
}
