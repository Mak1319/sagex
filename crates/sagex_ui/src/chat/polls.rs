//! User-created polls: creation sheet + local single-choice voting.
//!
//! Offline-first (votes live on the message, like everything else local);
//! server sync lands with the other endpoints later.

use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt as _,
    button::{Button, ButtonVariants},
    input::Input,
    label::Label,
    v_flex,
};

use super::{
    ChatApp,
    model::{Message, MessageKind, MessageStatus},
};

impl ChatApp {
    pub fn open_poll_sheet(&mut self, cx: &mut Context<Self>) {
        self.show_poll = true;
        self.show_profile = false;
        self.show_new_chat = false;
        self.show_group_info = false;
        self.close_menus();
        cx.notify();
    }

    /// Post the poll from the sheet inputs (question + ≥2 options).
    pub fn create_poll(&mut self, cx: &mut Context<Self>) {
        let q = self.poll.q.read(cx).value().trim().to_string();
        let opts: Vec<String> = self
            .poll
            .opts
            .iter()
            .take(self.poll_n)
            .map(|o| o.read(cx).value().trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if q.is_empty() {
            self.notice = Some("Give the poll a question.".to_string());
            cx.notify();
            return;
        }
        if opts.len() < 2 {
            self.notice = Some("Add at least two options.".to_string());
            cx.notify();
            return;
        }
        let n = opts.len();
        let id = self.next_msg;
        self.next_msg += 1;
        let my_name = self.my_name();
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == self.active_id) {
            c.messages.push(Message {
                id,
                sender: my_name,
                link: None,
                text: q.clone(),
                time: "now".to_string(),
                mine: true,
                date: "Today".to_string(),
                kind: MessageKind::Poll {
                    question: q,
                    options: opts,
                    votes: vec![0; n],
                    my_vote: None,
                },
                reactions: vec![],
                ticks: MessageStatus::Sent,
                deleted: false,
                server_id: None,
                attachment: None,
            });
            c.last_time = "now".to_string();
        }
        self.show_poll = false;
        cx.notify();
    }

    /// Single-choice vote: tap to vote, tap another to move the vote.
    pub fn cast_vote(&mut self, mid: usize, opt: usize, cx: &mut Context<Self>) {
        let mut changed = false;
        for c in self.chats.iter_mut() {
            if let Some(m) = c.messages.iter_mut().find(|m| m.id == mid) {
                if let MessageKind::Poll {
                    options,
                    votes,
                    my_vote,
                    ..
                } = &mut m.kind
                {
                    if opt >= options.len() {
                        break;
                    }
                    if *my_vote == Some(opt) {
                        break; // already voted here
                    }
                    if votes.len() != options.len() {
                        *votes = vec![0; options.len()];
                    }
                    if let Some(prev) = my_vote.take()
                        && let Some(v) = votes.get_mut(prev)
                    {
                        *v = v.saturating_sub(1);
                    }
                    if let Some(v) = votes.get_mut(opt) {
                        *v = v.saturating_add(1);
                    }
                    *my_vote = Some(opt);
                    changed = true;
                }
                break;
            }
        }
        if changed {
            cx.notify();
        }
    }

    pub(super) fn render_poll_sheet(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let n_opts = self
            .poll_n
            .clamp(super::PollInputs::MIN_OPTS, super::PollInputs::MAX_OPTS);
        let mut rows = v_flex().gap_2().child(Label::new("Options").text_sm());
        for (ix, opt) in self.poll.opts.iter().take(n_opts).enumerate() {
            // only the last visible row carries × (removes from the end,
            // so no text ever shifts between rows)
            let removable = n_opts > super::PollInputs::MIN_OPTS && ix + 1 == n_opts;
            rows = rows.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(opt).cleanable(true).w_full()),
                    )
                    .when(removable, |t| {
                        t.child(
                            Button::new(("poll-opt-rm", ix))
                                .ghost()
                                .small()
                                .icon(Icon::empty().path("icons/x.svg"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.poll_n = this
                                        .poll_n
                                        .saturating_sub(1)
                                        .max(super::PollInputs::MIN_OPTS);
                                    cx.notify();
                                })),
                        )
                    }),
            );
        }
        if n_opts < super::PollInputs::MAX_OPTS {
            rows = rows.child(
                Button::new("poll-opt-add")
                    .ghost()
                    .small()
                    .icon(Icon::empty().path("icons/plus.svg"))
                    .label("Add option")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.poll_n = (this.poll_n + 1).min(super::PollInputs::MAX_OPTS);
                        cx.notify();
                    })),
            );
        }
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .p_6()
            .bg(theme.background)
            .child(
                div()
                    .w_full()
                    .max_w(px(480.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_6()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(
                        div()
                            .text_lg()
                            .font_bold()
                            .text_color(theme.foreground)
                            .child("Create a poll"),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new("Question").text_sm())
                            .child(Input::new(&self.poll.q).cleanable(true).w_full()),
                    )
                    .child(rows)
                    .child(
                        div().text_xs().text_color(theme.muted_foreground).child(
                            "Single choice · tap an option to vote, tap another to move it.",
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap_2()
                            .w_full()
                            .child(Button::new("poll-cancel").ghost().label("Cancel").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.show_poll = false;
                                    cx.notify();
                                }),
                            ))
                            .child(
                                Button::new("poll-create")
                                    .primary()
                                    .label("Create poll")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.create_poll(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn vote_arithmetic() {
        // mirrors cast_vote: move a vote, totals stay consistent
        let mut votes = vec![2u32, 1];
        let mut my: Option<usize> = Some(0);
        let opt = 1;
        if let Some(prev) = my.take() {
            votes[prev] = votes[prev].saturating_sub(1);
        }
        votes[opt] = votes[opt].saturating_add(1);
        my = Some(opt);
        assert_eq!(votes, vec![1, 2]);
        assert_eq!(my, Some(1));
        assert_eq!(votes.iter().sum::<u32>(), 3);
    }
}
