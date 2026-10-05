use gpui::{Context, Div, Entity, div, prelude::*, px};

use crate::{
    Root,
    app::AppState,
    components::{Icon, IconButton, IconKind},
    models::Route,
    theme::palette as p,
};

/// Far-left 64px rail: logo / CHATS / system toggles / settings.
/// Route-aware: icons navigate.
pub fn rail(root: &Entity<Root>, s: &AppState, _cx: &mut Context<Root>) -> Div {
    let active_chats = matches!(s.route, Route::Channel(_));
    let r1 = root.clone();
    let r2 = root.clone();
    let mut chats = IconButton::new(IconKind::Chats);
    if active_chats {
        chats = chats.active();
    }
    chats = chats.on_click(move |_, _, cx| {
        let _ = r1.update(cx, |this, cx| {
            this.state
                .navigate(Route::Channel(crate::models::ChannelId(4)));
            cx.notify();
        });
    });
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_between()
        .w(px(64.0))
        .h_full()
        .bg(p::panel())
        .border_r_1()
        .border_color(p::hairline())
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(Icon::new(IconKind::Home).size(22.0))
                .child(chats),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(IconButton::new(IconKind::Moon))
                .child(IconButton::new(IconKind::Sun))
                .child(IconButton::new(IconKind::Monitor)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(
                    IconButton::new(IconKind::Settings).on_click(move |_, _, cx| {
                        let _ = r2.update(cx, |this, cx| {
                            this.state.navigate(Route::Settings);
                            cx.notify();
                        });
                    }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(p::ink_faint())
                        .child("SET".to_owned()),
                ),
        )
}
