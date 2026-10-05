use gpui::{IntoElement, Render, Window, div, prelude::*};

use crate::{
    components::{AttachmentCard, Badge, CodeBlock, MessageActionBar, SchematicCard, avatar_el},
    models::{Message, MsgBody},
    theme::palette as p,
};

/// One chat-stream entry. Real GPUI view (`impl Render`).
pub struct MessageCard {
    pub message: Message,
}

impl Render for MessageCard {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let m = &self.message;
        let initial = m.author.chars().next().unwrap_or('?').to_string();
        let body: gpui::AnyElement = match &m.body {
            MsgBody::Text(t) => div()
                .text_sm()
                .text_color(p::ink())
                .child(t.to_string())
                .into_any_element(),
            MsgBody::Schematic { title, lines } => SchematicCard {
                title: title.to_string(),
                lines: lines.iter().map(|l| l.to_string()).collect(),
            }
            .into_any_element(),
            MsgBody::Code { file, lang, code } => CodeBlock {
                file: file.to_string(),
                lang: lang.to_string(),
                code: code.to_string(),
            }
            .into_any_element(),
            MsgBody::Attachment { name, meta } => AttachmentCard {
                name: name.to_string(),
                meta: meta.to_string(),
            }
            .into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .p_2()
            .gap_1()
            .border_1()
            .border_color(p::hairline())
            .bg(p::panel())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(avatar_el(&initial, m.mine))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .text_sm()
                                            .child(m.author.to_string())
                                            .child(Badge::new(m.badge)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(p::ink_faint())
                                            .child(m.time.to_string()),
                                    ),
                            ),
                    )
                    .child(MessageActionBar),
            )
            .child(body)
    }
}
