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
use std::rc::Rc;

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
            MessageKind::Poll {
                question,
                options,
                votes,
                my_vote,
            } => BubbleContent::Poll {
                question: question.clone(),
                options: options.clone(),
                votes: votes.clone(),
                my_vote: *my_vote,
                tag: 0,
                on_vote: None,
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
        // Real attachments bypass the placeholder kinds: full File bubble.
        let content = match &m.attachment {
            Some(att) => {
                let view = cx.entity();
                let open_path = att.path.clone();
                let prev = super::attach::PreviewSel {
                    path: att.path.clone(),
                    name: att.name.clone(),
                };
                let is_image = att.kind == super::files::FileKind::Image;
                BubbleContent::File {
                    tag: mid,
                    thumb: is_image.then(|| att.path.to_string_lossy().into_owned()),
                    name: att.name.clone(),
                    size: super::files::fmt_size(att.size),
                    icon: att.kind.icon().to_string(),
                    locked: att.locked,
                    on_open: Some(Rc::new(
                        move |_: &gpui::ClickEvent, _: &mut gpui::Window, _: &mut gpui::App| {
                            ChatApp::open_path(&open_path);
                        },
                    )),
                    on_preview: is_image.then(|| {
                        let handler: crate::component::FileHandler = Rc::new(
                            move |_: &gpui::ClickEvent,
                                  _: &mut gpui::Window,
                                  cx: &mut gpui::App| {
                                view.update(cx, |this: &mut ChatApp, cx| {
                                    this.preview = Some(prev.clone());
                                    cx.notify();
                                });
                            },
                        );
                        handler
                    }),
                }
            }
            None => match &m.kind {
                super::model::MessageKind::Poll {
                    question,
                    options,
                    votes,
                    my_vote,
                } => {
                    let view = cx.entity();
                    let handler: crate::component::VoteHandler = Rc::new(
                        move |opt: usize,
                              _: &gpui::ClickEvent,
                              _: &mut gpui::Window,
                              cx: &mut gpui::App| {
                            view.update(cx, |this: &mut ChatApp, cx| {
                                this.cast_vote(mid, opt, cx);
                            });
                        },
                    );
                    BubbleContent::Poll {
                        question: question.clone(),
                        options: options.clone(),
                        votes: votes.clone(),
                        my_vote: *my_vote,
                        tag: mid,
                        on_vote: Some(handler),
                    }
                }
                _ => Self::bubble_content(m),
            },
        };
        let mut bubble = ChatBubble::new(m.mine, content, m.time.clone())
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
