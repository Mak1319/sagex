//! Composer zone: notice pill + message composer + attach/emoji panels.

use gpui::{Context, IntoElement, ParentElement, Styled, Window, div, prelude::FluentBuilder};

use super::{
    ChatApp,
    emoji::{EMOJI_ALIASES, EMOJI_CATEGORIES},
    model::{MessageKind, voice_bars},
};
use crate::component::{AttachMenu, EmojiPanel, MessageComposer, NoticePill, ReplyBar};

impl ChatApp {
    pub(super) fn render_composer_zone(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let empty = self.composer.read(cx).value().trim().is_empty();
        div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .when(self.show_attach, |t| {
                t.child(AttachMenu::new().on_select(cx.listener(
                    move |this, ix: &usize, _window, cx| {
                        let ix = *ix;
                        let id = this.active_id;
                        let seed = this.next_msg;
                        let bodies = [
                            "📄 Proposal.pdf",
                            "🖼️ Photo",
                            "📷 Photo",
                            "🎵 Voice note (1:12)",
                            "👤 Ada Lovelace",
                            "📊 Poll: Weekend plan?",
                            "📅 Team dinner",
                            "🦊",
                        ];
                        let kind = match ix {
                            0 => MessageKind::Document {
                                name: "Proposal.pdf".to_string(),
                                size: "2.4 MB".to_string(),
                            },
                            1 => MessageKind::Image {
                                caption: "Beach day 🏖️".to_string(),
                            },
                            2 => MessageKind::Image {
                                caption: String::new(),
                            },
                            3 => MessageKind::Voice {
                                duration: "1:12".to_string(),
                                bars: voice_bars(seed),
                            },
                            4 => MessageKind::Contact {
                                name: "Ada Lovelace".to_string(),
                                phone: "+91 98765 43210".to_string(),
                            },
                            5 => MessageKind::Poll {
                                question: "Weekend plan?".to_string(),
                                options: vec![
                                    ("Trek ⛰️".to_string(), 3),
                                    ("Movie 🎬".to_string(), 5),
                                    ("Stay in 🏠".to_string(), 1),
                                ],
                            },
                            6 => MessageKind::Event {
                                title: "Team dinner".to_string(),
                                when: "Sun, 7 pm".to_string(),
                            },
                            _ => MessageKind::Sticker {
                                glyph: "🦊".to_string(),
                            },
                        };
                        this.push_kind(id, kind, bodies[ix].to_string());
                        this.show_attach = false;
                        cx.notify();
                    },
                )))
            })
            .when(self.show_emoji, |t| {
                t.child(
                    EmojiPanel::new(
                        &self.emoji_search,
                        &self.emoji_scroll,
                        EMOJI_CATEGORIES,
                        EMOJI_ALIASES,
                    )
                    .tab(self.emoji_tab)
                    .show_gif(self.show_gif)
                    .on_tab(cx.listener(|this, ix: &usize, _window, cx| {
                        this.emoji_tab = *ix;
                        this.show_gif = false;
                        cx.notify();
                    }))
                    .on_pick(cx.listener(|this, glyph: &String, window, cx| {
                        let cur = this.composer.read(cx).value().to_string();
                        this.composer
                            .update(cx, |s, cx| s.set_value(format!("{cur}{glyph}"), window, cx));
                        cx.notify();
                    }))
                    .on_gif(cx.listener(|this, show: &bool, _window, cx| {
                        this.show_gif = *show;
                        cx.notify();
                    })),
                )
            })
            // reply preview bar (menu Reply actions)
            .when_some(self.reply_to.clone(), |t, (sender, text, color)| {
                t.child(ReplyBar::new(sender, color, text).on_close(cx.listener(
                    |this, _, _, cx| {
                        this.reply_to = None;
                        cx.notify();
                    },
                )))
            })
            // transient action feedback pill (menu actions)
            .when_some(self.notice.clone(), |t, msg| {
                t.child(
                    NoticePill::new(msg).on_dismiss(cx.listener(|this, _, _, cx| {
                        this.notice = None;
                        cx.notify();
                    })),
                )
            })
            .child(
                MessageComposer::new(&self.composer, empty)
                    .on_attach(cx.listener(|this, _, _, cx| {
                        this.show_attach = !this.show_attach;
                        this.show_emoji = false;
                        cx.notify();
                    }))
                    .on_emoji(cx.listener(|this, _, _, cx| {
                        this.show_emoji = !this.show_emoji;
                        this.show_attach = false;
                        cx.notify();
                    }))
                    .on_send(cx.listener(|this, _, window, cx| {
                        if this.composer.read(cx).value().trim().is_empty() {
                            let id = this.active_id;
                            let seed = this.next_msg;
                            this.push_kind(
                                id,
                                MessageKind::Voice {
                                    duration: "0:30".to_string(),
                                    bars: voice_bars(seed),
                                },
                                "🎤 Voice note (0:30)".to_string(),
                            );
                            cx.notify();
                            return;
                        }
                        // fold an active reply quote into the sent text
                        if let Some((sender, text, _)) = this.reply_to.clone() {
                            let cur = this.composer.read(cx).value().to_string();
                            this.composer.update(cx, |s, cx| {
                                s.set_value(format!("↩ {sender}: {text} {cur}"), window, cx)
                            });
                            this.reply_to = None;
                        }
                        this.send_composer(window, cx);
                    })),
            )
    }
}
