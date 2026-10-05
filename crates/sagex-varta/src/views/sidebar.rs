use gpui::{Context, Div, Entity, div, prelude::*, px};

use crate::{
    Root,
    app::AppState,
    components::{ChannelRow, IconKind, SearchInput, SectionHead, icon_el},
    models::Route,
    theme::palette as p,
};

/// Conversation list: search + pinned + recent + filter chips.
pub fn sidebar(root: &Entity<Root>, s: &AppState, cx: &mut Context<Root>) -> Div {
    let q = s.search.to_lowercase();
    let rows: Vec<_> = s
        .channels
        .iter()
        .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q))
        .cloned()
        .collect();
    let active = match s.route {
        Route::Channel(id) => id.0,
        _ => u32::MAX,
    };
    let (search, search_focus) = (s.search.clone(), s.search_focus);

    let (pinned, recent): (Vec<_>, Vec<_>) = rows.into_iter().partition(|c| c.pinned);
    let mut feed = div()
        .id("channel-feed")
        .flex()
        .flex_col()
        .flex_1()
        .overflow_y_scroll();
    feed = feed.child(SectionHead::new(format!("PINNED ({})", pinned.len())));
    for ch in pinned {
        let r = root.clone();
        let id = ch.id;
        let is_active = id.0 == active;
        feed = feed.child(cx.new(move |_| {
            let rr = r.clone();
            let mut row = ChannelRow::new(ch);
            if is_active {
                row = row.active();
            }
            row.on_click(move |_, _, cx| {
                let _ = rr.update(cx, |this, cx| {
                    this.state.navigate(Route::Channel(id));
                    cx.notify();
                });
            })
        }));
    }
    feed = feed.child(SectionHead::new("RECENT  ·  FEED: LIVE"));
    for ch in recent {
        let r = root.clone();
        let id = ch.id;
        let is_active = id.0 == active;
        feed = feed.child(cx.new(move |_| {
            let rr = r.clone();
            let mut row = ChannelRow::new(ch);
            if is_active {
                row = row.active();
            } else {
                row = row.on_click(move |_, _, cx| {
                    let _ = rr.update(cx, |this, cx| {
                        this.state.navigate(Route::Channel(id));
                        cx.notify();
                    });
                });
            }
            row
        }));
    }

    let hs = root.clone();
    let search_row = div()
        .id("channel-search")
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            let _ = hs.update(cx, |this, cx| {
                this.state.search_focus = true;
                cx.notify();
            });
        })
        .child(SearchInput {
            value: search,
            focused: search_focus,
        });

    div()
        .flex()
        .flex_col()
        .w(px(300.0))
        .h_full()
        .bg(p::panel())
        .border_r_1()
        .border_color(p::hairline())
        .child(
            div()
                .p_3()
                .gap_1()
                .flex()
                .flex_col()
                .border_b_1()
                .border_color(p::hairline())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_xs()
                        .child(
                            div()
                                .text_color(p::primary())
                                .child("DIR // ACTIVE CHANNELS".to_owned()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(p::ink_dim())
                                .child("LOC: 0x02A".to_owned()),
                        ),
                )
                .child(search_row)
                .child(
                    div()
                        .text_xs()
                        .text_color(p::ink_faint())
                        .child("⌘K to search · / to focus".to_owned()),
                ),
        )
        .child(feed)
        .child(
            div()
                .flex()
                .gap_2()
                .p_2()
                .border_t_1()
                .border_color(p::hairline())
                .child(icon_el(IconKind::Unread, 15.0, false, false))
                .child(icon_el(IconKind::Groups, 15.0, false, false))
                .child(icon_el(IconKind::Archived, 15.0, false, false))
                .child(icon_el(IconKind::Delete, 15.0, false, true)),
        )
}
