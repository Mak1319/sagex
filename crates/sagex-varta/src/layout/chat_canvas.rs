//! Chat canvas — 3-row system: top bar, adjustable chat section, composer.
//!
//! Manual `gpui-base` expansion only. Owns the composer `TextareaState`,
//! the license-deck flag, and the deck's interactive state (expanded
//! accordion row, collapsed section, checked recipients).

use gpui_kit::base::input::InputState;
use gpui_kit::component::input::TextareaState;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::components::cards::{FileCard, KindBox, MetaSeg, ScrubberBar, WaveformBar};
use crate::components::chat_composer::ChatComposer;
use crate::components::chat_message::{ChatMessage, MessageBadge, MessageSide};
use crate::components::license_composer::{LicenseComposer, RECIPIENT_COUNT};
use crate::layout::chat_top_bar::ChatTopBar;
use crate::seed::{SeedCard, SeedMessage, SeedSide, load_messages};
use crate::theme;

pub struct ChatCanvas {
    composer: Entity<TextareaState>,
    filter: Entity<InputState>,
    _filter_sub: Subscription,
    messages: Vec<SeedMessage>,
    show_license: bool,
    expanded: Option<usize>,
    deck_collapsed: bool,
    access_collapsed: bool,
    checked: [bool; RECIPIENT_COUNT],
}

/// Temporary debug default: deck visible on boot. Flip to false to restore.
const DEBUG_SHOW_LICENSE: bool = false;

impl ChatCanvas {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx: &mut Context<TextareaState>| {
            let mut state = TextareaState::new(window, cx).auto_grow(1, 6);
            state.set_placeholder("Type a message...", window, cx);
            state
        });
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter recipients…"));
        // Re-render the deck on every keystroke so the recipient list
        // live-filters. The engine notifies its observers on edit.
        let _filter_sub = cx.observe(&filter, |_, _, cx| cx.notify());
        Self {
            composer,
            filter,
            _filter_sub,
            messages: load_messages(),
            show_license: DEBUG_SHOW_LICENSE,
            expanded: None,
            deck_collapsed: false,
            access_collapsed: false,
            checked: [true, true, true, false, false],
        }
    }

    fn set_license(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_license = show;
        cx.notify();
    }

    fn toggle_attachment(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.expanded = if self.expanded == Some(ix) {
            None
        } else {
            Some(ix)
        };
        cx.notify();
    }

    fn toggle_collapsed(&mut self, cx: &mut Context<Self>) {
        self.deck_collapsed = !self.deck_collapsed;
        cx.notify();
    }

    fn toggle_access(&mut self, cx: &mut Context<Self>) {
        self.access_collapsed = !self.access_collapsed;
        cx.notify();
    }

    fn toggle_recipient(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < RECIPIENT_COUNT {
            self.checked[ix] = !self.checked[ix];
            cx.notify();
        }
    }
}

fn seed_card(seed: &SeedCard) -> FileCard {
    let mut card = FileCard::new(seed.kind, seed.filename.clone()).id(seed.filename.clone());
    if let Some(chip) = seed.chip.clone() {
        card = card.chip(chip);
    }
    card = card.meta(
        seed.meta
            .iter()
            .map(|seg| {
                let mut out = MetaSeg::new(seg.text.clone())
                    .color(seg.color.map(|c| c.0).unwrap_or(theme::colors::MUTED));
                if seg.bold {
                    out = out.bold();
                }
                out
            })
            .collect(),
    );
    if let Some(tag) = seed.thumb_tag.clone() {
        card = card.leading(KindBox::new(seed.kind).size(40.0).tag(tag));
    }
    if let Some(primary) = seed.primary.clone() {
        card = card.primary(primary.label, |_, _, _| {});
        if let Some(icon) = primary.icon {
            card = card.primary_icon(icon);
        }
    }
    if let Some(icon) = seed.icon_action.clone() {
        card = card.icon_action(icon, |_, _, _| {});
    }
    if let Some(footer) = seed.footer.clone() {
        if let Some(scrubber) = footer.scrubber {
            card = card.footer(ScrubberBar::new(
                scrubber.progress,
                scrubber.elapsed,
                scrubber.total,
            ));
        } else if let Some(waveform) = footer.waveform {
            card = card.footer(WaveformBar::new(
                waveform.bars,
                waveform.played,
                waveform.elapsed,
                waveform.total,
            ));
        }
    }
    card
}

fn seed_message(seed: &SeedMessage) -> ChatMessage {
    let side = match seed.side {
        SeedSide::Recipient => MessageSide::Recipient,
        SeedSide::Sender => MessageSide::Sender,
    };
    let mut message = ChatMessage::new(side, seed.name.clone())
        .initial(seed.initial.clone())
        .avatar(seed.avatar_bg.0, seed.avatar_fg.0)
        .time(seed.time.clone())
        .body(seed.body.clone());
    if let Some(border) = seed.avatar_border {
        message = message.avatar_border(border.0);
    }
    if let Some(badge) = seed.badge.clone() {
        message = message.badge(MessageBadge::new(
            badge.text,
            badge.fg.0,
            badge.bg.0,
            badge.border.0,
        ));
    }
    if let Some(card) = seed.attachment.clone() {
        message = message.attachment(seed_card(&card));
    }
    message
}

impl Render for ChatCanvas {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let closer = cx.entity().clone();
        let attacher = cx.entity().clone();
        let collapser = cx.entity().clone();
        let accesser = cx.entity().clone();
        let checker = cx.entity().clone();
        let expanded = self.expanded;
        let collapsed = self.deck_collapsed;
        let access_collapsed = self.access_collapsed;
        let checked = self.checked;
        let filter = self.filter.clone();
        div()
            .id(ElementId::Name("chat-canvas".into()))
            .flex()
            .flex_col()
            .flex_1()
            .size_full()
            .overflow_hidden()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .child(ChatTopBar::new(
                "Marketing",
                "12 members · 3 online",
            ))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .relative()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .w_full()
                            .h_full()
                            .id(ElementId::Name("chat-messages".into()))
                            .overflow_y_scroll()
                            .scrollbar_width(px(0.0))
                            .px_4()
                            .py_3()
                            .children(self.messages.iter().map(seed_message)),
                    )
                    .when(self.show_license, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .flex()
                                .flex_row()
                                .justify_center()
                                .items_end()
                                .p_3()
                                .child(
                                    LicenseComposer::new("Protected Attachment Clearance")
                                        .expanded(expanded)
                                        .on_attach_toggle(
                                            move |ix, _, _window, cx: &mut App| {
                                                attacher.update(cx, |view, cx| {
                                                    view.toggle_attachment(ix, cx)
                                                });
                                            },
                                        )
                                        .collapsed(collapsed)
                                        .on_collapsed_toggle(
                                            move |_, _window, cx: &mut App| {
                                                collapser.update(cx, |view, cx| {
                                                    view.toggle_collapsed(cx)
                                                });
                                            },
                                        )
                                        .access_collapsed(access_collapsed)
                                        .on_access_toggle(
                                            move |_, _window, cx: &mut App| {
                                                accesser.update(cx, |view, cx| {
                                                    view.toggle_access(cx)
                                                });
                                            },
                                        )
                                        .filter(filter)
                                        .checked(checked)
                                        .on_recipient_toggle(
                                            move |ix, _, _window, cx: &mut App| {
                                                checker.update(cx, |view, cx| {
                                                    view.toggle_recipient(ix, cx)
                                                });
                                            },
                                        )
                                        .on_close(move |_, _window, cx: &mut App| {
                                            closer.update(cx, |view, cx| view.set_license(false, cx));
                                        }),
                                ),
                        )
                    }),
            )
            .child(
                ChatComposer::new(self.composer.clone()).on_attach(move |_, _window, cx: &mut App| {
                    view.update(cx, |view, cx| view.set_license(true, cx));
                }),
            )
    }
}
