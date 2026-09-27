//! Message rows: avatar + tail + bubble + chips + anchored menu.
//!
//! Layout adapter over `component::bubble`; message-type mapping lives here.

use gpui::{Context, IntoElement, ParentElement, Styled, div, prelude::FluentBuilder, px};
use gpui_component::{ActiveTheme, Sizable, Size};

use super::{
    ChatApp,
    model::{Message, MessageKind, sender_color, sender_initials},
};
use crate::component::{
    Avatar, BubbleContent, ChatBubble, LinkCard, OUTGOING_DARK, OUTGOING_LIGHT, ReactionChips,
    Tail, Tick,
};

impl ChatApp {
    fn bubble_content(m: &Message) -> BubbleContent {
        match &m.kind {
            MessageKind::Text => BubbleContent::Text {
                text: m.text.clone(),
                link: m.link.clone().map(|l| LinkCard {
                    domain: l.domain,
                    title: l.title,
                    url: l.url,
                }),
            },
            MessageKind::Image { caption } => BubbleContent::Image {
                caption: caption.clone(),
            },
            MessageKind::Voice { duration, bars } => BubbleContent::Voice {
                duration: duration.clone(),
                bars: bars.clone(),
            },
            MessageKind::Document { name, size } => BubbleContent::Document {
                name: name.clone(),
                size: size.clone(),
            },
            MessageKind::Contact { name, phone, .. } => BubbleContent::Contact {
                name: name.clone(),
                phone: phone.clone(),
                color: sender_color(name),
                initials: sender_initials(name),
            },
            MessageKind::Poll { question, options } => BubbleContent::Poll {
                question: question.clone(),
                options: options.clone(),
            },
            MessageKind::Event { title, when } => BubbleContent::Event {
                title: title.clone(),
                when: when.clone(),
            },
            MessageKind::Sticker { .. } => BubbleContent::Text {
                text: m.text.clone(),
                link: None,
            },
        }
    }

    pub(super) fn render_bubble(
        &mut self,
        cx: &mut Context<Self>,
        m: &Message,
        is_group: bool,
        show_avatar: bool,
    ) -> impl IntoElement {
        let mid = m.id;
        // Deleted messages stay as a record (WhatsApp style), no menu.
        if m.deleted {
            let mid = m.id;
            return div()
                .flex()
                .w_full()
                .mt(px(5.))
                .mb(px(5.))
                .when(m.mine, |t| t.justify_end())
                .when(!m.mine, |t| t.justify_start())
                .child(
                    div().relative().max_w(px(520.)).child(
                        ChatBubble::new(m.mine, BubbleContent::Deleted, m.time.clone())
                            .menu_tag(mid)
                            .on_menu(cx.listener(move |this, _, _, cx| {
                                this.close_menus();
                                this.msg_menu_at = this.bubble_anchor_for(mid);
                                this.msg_menu = Some(mid);
                                cx.notify();
                            })),
                    ),
                );
        }
        // Stickers float without a bubble.
        if let MessageKind::Sticker { glyph } = &m.kind {
            return div()
                .flex()
                .w_full()
                .mt(px(5.))
                .mb(px(5.))
                .when(m.mine, |t| t.justify_end())
                .when(!m.mine, |t| t.justify_start())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap_1()
                        .child(div().text_size(px(48.)).child(glyph.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(m.time.clone()),
                        ),
                );
        }
        div()
            .flex()
            .w_full()
            .relative()
            .items_start()
            .gap_2()
            // explicit vertical rhythm: 5px above + below every row
            // (does not rely on flex-gap support).
            .mt(px(5.))
            .mb(px(5.))
            .when(m.mine, |t| t.justify_end())
            .when(!m.mine, |t| t.justify_start())
            // sender avatar: only the last of a consecutive sender run
            .when(show_avatar, |t| {
                t.child(
                    Avatar::new(sender_initials(&m.sender))
                        .color(sender_color(&m.sender))
                        .with_size(Size::Small),
                )
            })
            .child(
                div()
                    .relative()
                    .max_w(px(529.))
                    // tail sits beside the bubble top in normal flow:
                    // flush alignment, nothing to clip or misplace.
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .when(!m.mine, |t| t.child(Tail::new(false, cx.theme().muted)))
                            .child(Self::bubble_for(
                                cx,
                                m,
                                is_group,
                                mid,
                                self.pinned.contains(&m.id),
                            ))
                            .when(m.mine, |t| {
                                t.child(Tail::new(
                                    true,
                                    if cx.theme().is_dark() {
                                        gpui::rgb(OUTGOING_DARK)
                                    } else {
                                        gpui::rgb(OUTGOING_LIGHT)
                                    },
                                ))
                            }),
                    )
                    .child(ReactionChips::new(
                        m.reactions.clone(),
                        self.starred.contains(&m.id),
                        m.mine,
                    )),
            )
    }

    fn bubble_for(
        cx: &mut Context<Self>,
        m: &Message,
        is_group: bool,
        mid: usize,
        pinned: bool,
    ) -> impl IntoElement {
        use super::model::MessageStatus;
        let ticks = m.mine.then_some(match m.ticks {
            MessageStatus::Sent => Tick::Sent,
            MessageStatus::Delivered => Tick::Delivered,
            MessageStatus::Read => Tick::Read,
        });
        let mut bubble = ChatBubble::new(m.mine, Self::bubble_content(m), m.time.clone())
            .pinned(pinned)
            .ticks(ticks)
            .menu_tag(mid);
        if !m.mine && is_group {
            bubble = bubble.sender(m.sender.clone(), sender_color(&m.sender));
        }
        bubble.on_menu(cx.listener(move |this, _, _, cx| {
            this.close_menus();
            this.msg_menu_at = this.bubble_anchor_for(mid);
            this.msg_menu = Some(mid);
            cx.notify();
        }))
    }
}
