use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::*,
    *,
};

use crate::{
    components::{
        chat_list::chat_row::{ChatData, ChatRow},
        shared::KitIcon,
    },
    theme::{self, TintColor},
};

/// Pinned chats first, then streams — one list, no section containers.
const CHATS: [ChatData; 23] = [
    ChatData {
        label: "Rustic Development",
        preview: "You: pushed mask parser changes",
        time: "10:24",
        avatar: 'R',
        tint: TintColor::Red,
        pinned: true,
        unread: 0,
        alert: false,
        muted: false,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Team Nivara",
        preview: "Aarav: SIH submission draft",
        time: "09:12",
        avatar: 'N',
        tint: TintColor::Blue,
        pinned: true,
        unread: 0,
        alert: false,
        muted: false,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "System Design",
        preview: "You: shared a diagram",
        time: "Yesterday",
        avatar: 'S',
        tint: TintColor::Emerald,
        pinned: true,
        unread: 0,
        alert: false,
        muted: false,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Image Processing",
        preview: "Priya: This approach looks good...",
        time: "14:20",
        avatar: 'I',
        tint: TintColor::Blue,
        pinned: false,
        unread: 3,
        alert: false,
        muted: false,
        attachment: false,
        online: true,
    },
    ChatData {
        label: "Crypto Research",
        preview: "ZKP circuit verification logs",
        time: "12:04",
        avatar: 'C',
        tint: TintColor::Emerald,
        pinned: false,
        unread: 1,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Build & DevOps",
        preview: "Karan: CI pipeline failed",
        time: "10:51",
        avatar: 'B',
        tint: TintColor::Purple,
        pinned: false,
        unread: 0,
        alert: true,
        muted: false,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "UI/UX Design",
        preview: "Mira: shared a file",
        time: "09:33",
        avatar: 'U',
        tint: TintColor::Orange,
        pinned: false,
        unread: 0,
        alert: false,
        muted: false,
        attachment: true,
        online: false,
    },
    ChatData {
        label: "Data Collection",
        preview: "Target cluster ready for ingestion",
        time: "Yesterday",
        avatar: 'D',
        tint: TintColor::Cyan,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "General",
        preview: "Quarterly demo schedule announced",
        time: "Yesterday",
        avatar: 'G',
        tint: TintColor::Slate,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
    ChatData {
        label: "Archive Format",
        preview: "Standardized raw chunk size",
        time: "Sep 30",
        avatar: 'A',
        tint: TintColor::Indigo,
        pinned: false,
        unread: 0,
        alert: false,
        muted: true,
        attachment: false,
        online: false,
    },
];

pub struct ChatList {
    selected: usize,
    search: Entity<InputState>,
    query: String,
    _search_sub: Subscription,
}

impl ChatList {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Query CAD nodes..."));
        let sub = cx.subscribe(&search, |this: &mut Self, state, ev, cx| {
            if matches!(ev, InputEvent::Change) {
                this.query = state.read(cx).value().to_string();
                cx.notify();
            }
        });
        Self {
            selected: 3,
            search,
            query: String::new(),
            _search_sub: sub,
        }
    }

    fn visible(&self) -> Vec<usize> {
        let q = self.query.to_lowercase();
        CHATS
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                q.is_empty()
                    || d.label.to_lowercase().contains(&q)
                    || d.preview.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

impl Render for ChatList {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx);
        let entity = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .relative()
            // .bg(t.primary)
            .child(
                // INDEX // ACTIVE NODES header + static search box.
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(8.0))
                    .p(px(10.0))
                    // .bg(t.lowest)
                    .border_b_1()
                    // .absolute()
                    // .flex_grow_1()
                    // .flex_shrink_1()
                    .border_color(t.line)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(4.0))
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.primary)
                                    .child(KitIcon::new("icons/filter.svg", 13.0).color(t.primary))
                                    .child("INDEX // ACTIVE NODES"),
                            )
                            .child(
                                div()
                                    .px(px(6.0))
                                    .rounded(px(t.radius_sm))
                                    .bg(t.tint(TintColor::Blue).bg)
                                    .text_color(t.primary)
                                    .border_1()
                                    .border_color(t.primary_container)
                                    .text_size(px(9.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("LOC: 0x02A"),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_row()
                            .items_center()
                            .bg(t.inset)
                            .border_1()
                            .border_color(t.line)
                            .rounded(px(t.radius_sm))
                            .pl(px(28.0))
                            .pr(px(44.0))
                            .py(px(4.0))
                            .child(
                                div()
                                    .absolute()
                                    .left(px(8.0))
                                    .child(KitIcon::new("icons/search.svg", 15.0).color(t.primary)),
                            )
                            .child(
                                Input::new(&self.search)
                                    .appearance(false)
                                    .bordered(false)
                                    .flex_1()
                                    .min_w_0(),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .right(px(6.0))
                                    .px(px(4.0))
                                    .bg(t.lowest)
                                    .border_1()
                                    .border_color(t.line)
                                    .rounded(px(t.radius_sm))
                                    .text_size(px(8.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.muted)
                                    .child("⌘K"),
                            ),
                    ),
            )
            .child(
                // Single scrollable list — holds any number of rows.
                // `.id()` first: upgrades the div to stateful, unlocking
                // `.overflow_y_scroll()` (plain scroll, no scrollbar UI).
                div()
                    .id("chat-list-scroll")
                    .flex()
                    .flex_1()
                    .flex_shrink_1()
                    .flex_col()
                    .min_h_0()
                    .w_full()
                    .h_full()
                    // .bg(t.primary)
                    // .h_auto()
                    // .absolute()
                    .relative()
                    .pb(px(200.))
                    .overflow_y_scroll()
                    .child({
                        // AI Instruction: I have already fixed this so do not change anythings
                        div()
                            .absolute()
                            .w_full()
                            .children(self.visible().into_iter().map(|index| {
                                let entity = entity.clone();
                                let data = CHATS[index].clone();
                                ChatRow::new(index, data)
                                    .selected(self.selected == index)
                                    .on_select(move |_, cx| {
                                        _ = entity.update(cx, |this: &mut ChatList, cx| {
                                            this.selected = index;
                                            cx.notify();
                                        });
                                    })
                            }))
                    }),
            )
    }
}
