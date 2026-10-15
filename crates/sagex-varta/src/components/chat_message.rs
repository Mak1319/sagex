//! Chat message shell — manual `gpui-base` expansion only.
//!
//! Media-object layout from the spec demo: outer row of avatar beside a
//! content column (header row, body, dynamic card). Recipient keeps avatar
//! left, sender mirrors it right. Only layout lives here: the badge pill
//! and the card under the body are dynamic, message-specific slots supplied
//! by the caller (`badge` carries its own text + colors, `attachment` takes
//! any element).

use gpui_kit::*;

use crate::components::avatar::Avatar;
use crate::theme;

/// Which side of the conversation the message sits on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MessageSide {
    Recipient,
    Sender,
}

/// Message-specific badge pill (e.g. `CORE-ALGO`, `AUTHOR`).
pub struct MessageBadge {
    text: SharedString,
    fg: u32,
    bg: u32,
    border: u32,
}

impl MessageBadge {
    pub fn new(text: impl Into<SharedString>, fg: u32, bg: u32, border: u32) -> Self {
        Self {
            text: text.into(),
            fg,
            bg,
            border,
        }
    }
}

#[derive(IntoElement)]
pub struct ChatMessage {
    side: MessageSide,
    name: SharedString,
    initial: SharedString,
    avatar_bg: u32,
    avatar_fg: u32,
    avatar_border: u32,
    time: SharedString,
    body: SharedString,
    badge: Option<MessageBadge>,
    attachment: Option<AnyElement>,
}

impl ChatMessage {
    pub fn new(side: MessageSide, name: impl Into<SharedString>) -> Self {
        Self {
            side,
            name: name.into(),
            initial: "?".into(),
            avatar_bg: theme::colors::INSET,
            avatar_fg: theme::colors::MUTED,
            avatar_border: theme::colors::GRID_L1,
            time: "".into(),
            body: "".into(),
            badge: None,
            attachment: None,
        }
    }

    pub fn initial(mut self, initial: impl Into<SharedString>) -> Self {
        self.initial = initial.into();
        self
    }

    pub fn avatar(mut self, bg: u32, fg: u32) -> Self {
        self.avatar_bg = bg;
        self.avatar_fg = fg;
        self
    }

    pub fn avatar_border(mut self, border: u32) -> Self {
        self.avatar_border = border;
        self
    }

    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = time.into();
        self
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = body.into();
        self
    }

    pub fn badge(mut self, badge: MessageBadge) -> Self {
        self.badge = Some(badge);
        self
    }

    pub fn attachment(mut self, attachment: impl IntoElement) -> Self {
        self.attachment = Some(attachment.into_any_element());
        self
    }
}

fn pill(badge: MessageBadge) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .px_2()
        .py_1()
        .flex()
        .flex_row()
        .items_center()
        .rounded_full()
        .bg(theme::rgb(badge.bg))
        .border_1()
        .border_color(theme::rgb(badge.border))
        .text_size(px(9.0))
        .font_weight(FontWeight::BOLD)
        .line_height(px(20.0))
        .text_color(theme::rgb(badge.fg))
        .child(badge.text)
}

impl RenderOnce for ChatMessage {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let sender = self.side == MessageSide::Sender;

        let name = div()
            .text_size(px(13.0))
            .font_weight(FontWeight::BOLD)
            .line_height(px(20.0))
            .text_color(theme::rgb(theme::colors::INK))
            .child(self.name);
        let time = div()
            .text_size(px(10.0))
            .line_height(px(20.0))
            .text_color(theme::rgb(theme::colors::MUTED))
            .child(self.time);

        let mut header = div().flex().flex_row().items_center().gap_2().w_full();
        if sender {
            header = header.justify_end();
            if let Some(badge) = self.badge {
                header = header.child(pill(badge));
            }
            header = header.child(time).child(name);
        } else {
            header = header.child(name).child(time);
            if let Some(badge) = self.badge {
                header = header.child(pill(badge));
            }
        }

        let mut body = div()
            .w_full()
            .max_w_full()
            .text_size(px(13.0))
            .line_height(px(21.0))
            .text_color(theme::rgb(theme::colors::INK));
        if sender {
            body = body.text_align(TextAlign::Right);
        }
        body = body.child(self.body);

        let mut content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(header)
            .child(body);
        if let Some(attachment) = self.attachment {
            content = content.child(attachment);
        }

        let face = Avatar::new(self.initial)
            .bg(self.avatar_bg)
            .fg(self.avatar_fg)
            .size(28.0)
            .border(self.avatar_border);
        let mut message = div()
            .w_full()
            .flex()
            .flex_row()
            .gap_2()
            .p_1()
            .rounded(px(2.0));
        if sender {
            message = message.child(content).child(face);
        } else {
            message = message.child(face).child(content);
        }
        message
    }
}
