//! WhatsApp chat bubble: typed bodies, sender row, time row, menu chevron.
//!
//! Controlled component: every visual comes from props, actions leave
//! through `on_menu`. Reads the theme internally for bubble tints.

use gpui::{
    App, ClickEvent, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, img, prelude::FluentBuilder,
    px, relative, rgb,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt as _, button::Button, button::ButtonVariants,
};
use std::rc::Rc;

use super::colors::{ACCENT_GREEN, OUTGOING_DARK, OUTGOING_LIGHT};

/// Link preview card data.
#[derive(Clone)]
pub struct LinkCard {
    pub domain: String,
    pub title: String,
    pub url: String,
}

/// Message body variants (stickers float without a bubble — rendered by chat).
#[derive(Clone)]
pub enum BubbleContent {
    Text {
        text: String,
        link: Option<LinkCard>,
    },
    Deleted,
    Image {
        caption: String,
    },
    Voice {
        duration: String,
        bars: Vec<u32>,
    },
    Document {
        name: String,
        size: String,
    },
    /// Real local attachment: thumbnail or file row, wired open/preview.
    /// `tag` scopes element ids per message. Rendered by chat, styled here.
    File {
        tag: usize,
        thumb: Option<String>,
        name: String,
        size: String,
        icon: String,
        locked: bool,
        on_open: Option<super::FileHandler>,
        on_preview: Option<super::FileHandler>,
    },
    Contact {
        name: String,
        phone: String,
        color: u32,
        initials: String,
    },
    Poll {
        question: String,
        options: Vec<String>,
        votes: Vec<u32>,
        my_vote: Option<usize>,
        tag: usize,
        on_vote: Option<super::VoteHandler>,
    },
    Event {
        title: String,
        when: String,
    },
}

/// Read-receipt state for outgoing bubbles (WhatsApp ticks).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tick {
    /// Single grey tick: sent.
    #[default]
    Sent,
    /// Double grey tick: delivered.
    Delivered,
    /// Double blue tick: read.
    Read,
}

/// Full bubble: corners by side, optional sender row, body, time row.
#[derive(IntoElement)]
pub struct ChatBubble {
    style: StyleRefinement,
    mine: bool,
    content: BubbleContent,
    sender: Option<(String, u32)>,
    time: String,
    pinned: bool,
    ticks: Option<Tick>,
    menu_tag: Option<usize>,
    on_menu: Option<super::ClickHandler>,
}

impl ChatBubble {
    pub fn new(mine: bool, content: BubbleContent, time: impl Into<String>) -> Self {
        Self {
            style: StyleRefinement::default(),
            mine,
            content,
            sender: None,
            time: time.into(),
            pinned: false,
            ticks: None,
            menu_tag: None,
            on_menu: None,
        }
    }

    /// Group sender header: name + color.
    pub fn sender(mut self, name: impl Into<String>, color: u32) -> Self {
        self.sender = Some((name.into(), color));
        self
    }

    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// Read ticks after the timestamp (`None` = incoming, no ticks).
    pub fn ticks(mut self, ticks: Option<Tick>) -> Self {
        self.ticks = ticks;
        self
    }

    /// Show the ⌄ chevron; `menu_tag` keeps the button id unique.
    pub fn menu_tag(mut self, tag: usize) -> Self {
        self.menu_tag = Some(tag);
        self
    }

    pub fn on_menu(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_menu = Some(Rc::new(handler));
        self
    }
}

impl Styled for ChatBubble {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn chevron(tag: usize, on_menu: &Option<super::ClickHandler>) -> impl IntoElement {
    let mut btn = Button::new(("bubble-menu", tag))
        .ghost()
        .xsmall()
        .icon(gpui_component::IconName::ChevronDown);
    if let Some(handler) = on_menu.clone() {
        btn = btn.on_click(move |evt, window, cx: &mut App| (handler)(evt, window, cx));
    }
    btn
}

/// Deleted-message record: single inline row (icon + text + time +
/// chevron), matching WhatsApp. `mine` picks the phrasing.
fn deleted_row(
    mine: bool,
    time: &str,
    menu_tag: Option<usize>,
    on_menu: &Option<super::ClickHandler>,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .text_sm()
        .child("🚫")
        .child(div().flex_1().child(if mine {
            "You deleted this message".to_string()
        } else {
            "This message was deleted".to_string()
        }))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(time.to_string()),
        )
        .when_some(menu_tag, |t, tag| t.child(chevron(tag, on_menu)))
}

/// WhatsApp read ticks: single grey (sent), double grey (delivered),
/// double blue (read).
fn ticks(tick: Tick, muted: &gpui::Hsla) -> impl IntoElement {
    let (path, color): (&str, gpui::Hsla) = match tick {
        Tick::Sent => ("icons/check.svg", *muted),
        Tick::Delivered => ("icons/check-check.svg", *muted),
        Tick::Read => ("icons/check-check.svg", rgb(0x53bdeb).into()),
    };
    Icon::empty().path(path).size(px(14.)).text_color(color)
}

impl RenderOnce for ChatBubble {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let bubble = if self.mine {
            let base = if theme.is_dark() {
                div().bg(rgb(OUTGOING_DARK)).text_color(rgb(0xffffff))
            } else {
                div().bg(rgb(OUTGOING_LIGHT)).text_color(rgb(0x111b21))
            };
            base.rounded_tl(px(12.))
                .rounded_bl(px(12.))
                .rounded_br(px(12.))
            // .rounded_tr(px(2.))
        } else {
            div()
                .bg(theme.muted)
                .text_color(theme.foreground)
                .rounded_tr(px(12.))
                .rounded_br(px(12.))
                .rounded_bl(px(12.))
            // .rounded_tl(px(2.))
        };
        let inline_sender = !self.mine && self.sender.is_some();
        bubble.px_3().py_2().relative().child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .when(inline_sender, |t| {
                    let (name, color) = self.sender.clone().unwrap();
                    t.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgb(color))
                                    .child(name),
                            )
                            .when_some(self.menu_tag, |t, tag| {
                                t.child(chevron(tag, &self.on_menu))
                            }),
                    )
                })
                .child(match &self.content {
                    BubbleContent::Deleted => {
                        deleted_row(self.mine, &self.time, self.menu_tag, &self.on_menu, cx)
                            .into_any_element()
                    }
                    other => body(other, cx),
                })
                .when(!matches!(self.content, BubbleContent::Deleted), |t| {
                    t.child(
                        div()
                            .flex()
                            .justify_end()
                            .items_center()
                            .gap_1()
                            .when(self.pinned, |t| {
                                t.child(
                                    Icon::empty()
                                        .path("icons/pin.svg")
                                        .size(px(12.))
                                        .text_color(theme.muted_foreground),
                                )
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(self.time),
                            )
                            .when_some(self.ticks, |t, tick| {
                                t.child(ticks(tick, &theme.muted_foreground))
                            })
                            .when(!inline_sender, |t| {
                                t.child(match self.menu_tag {
                                    Some(tag) => chevron(tag, &self.on_menu).into_any_element(),
                                    None => div().into_any_element(),
                                })
                            }),
                    )
                }),
        )
    }
}

/// Body for one content variant (Deleted is rendered by `deleted_row`).
fn body(content: &BubbleContent, cx: &App) -> gpui::AnyElement {
    let theme = cx.theme().clone();
    match content {
        BubbleContent::Text { text, link } => div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_sm().child(text.clone()))
            .when_some(link.clone(), |t, link| {
                t.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .rounded_md()
                        .bg(theme.background)
                        .p_2()
                        .mt_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(link.title.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(link.domain.clone()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .underline()
                                .text_color(rgb(ACCENT_GREEN))
                                .child(link.url.clone()),
                        ),
                )
            })
            .into_any_element(),
        BubbleContent::Deleted => div().into_any_element(),
        BubbleContent::Image { caption } => div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .w(px(300.))
                    .h(px(180.))
                    .rounded_md()
                    .bg(theme.background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child(Icon::empty().path("icons/image.svg").large()),
            )
            .when(!caption.is_empty(), |t| {
                t.child(div().text_sm().child(caption.clone()))
            })
            .into_any_element(),
        BubbleContent::Voice { duration, bars } => {
            let total = bars.len().max(1);
            let mut row = div().flex().flex_row().items_end().gap(px(2.));
            for (ix, h) in bars.iter().enumerate() {
                let played = ix * 5 < total * 2;
                row = row.child(
                    div()
                        .w(px(3.))
                        .h(px(*h as f32))
                        .rounded_full()
                        .when(played, |t| t.bg(rgb(ACCENT_GREEN)))
                        .when(!played, |t| t.bg(theme.muted_foreground)),
                );
            }
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(32.))
                        .flex_shrink_0()
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.background)
                        .text_color(theme.foreground)
                        .child(Icon::empty().path("icons/play.svg").small()),
                )
                .child(
                    div().flex().flex_col().gap_1().child(row).child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(duration.clone()),
                    ),
                )
                .into_any_element()
        }
        BubbleContent::Document { name, size } => div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                div()
                    .size(px(40.))
                    .flex_shrink_0()
                    .rounded_md()
                    .bg(theme.background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child(Icon::empty().path("icons/file-text.svg")),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(div().text_sm().child(name.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(size.clone()),
                    ),
            )
            .child(
                Button::new("bubble-doc-dl")
                    .ghost()
                    .small()
                    .icon(Icon::empty().path("icons/download.svg"))
                    .on_click(|_, _, _| println!("document download")),
            )
            .into_any_element(),
        BubbleContent::File {
            tag,
            thumb,
            name,
            size,
            icon,
            locked,
            on_open,
            on_preview,
        } => {
            let mut body = div()
                .flex()
                .flex_col()
                .gap_2()
                .min_w(px(240.))
                .max_w(px(320.));
            // thumbnail (images) or icon row
            if let Some(t) = thumb {
                let frame = div()
                    .w(px(300.))
                    .h(px(180.))
                    .rounded_md()
                    .bg(theme.background)
                    .overflow_hidden()
                    .child(img(std::path::Path::new(t)).w_full().h_full());
                body = body.child(if let Some(preview) = on_preview.clone() {
                    div()
                        .id(("bfile-img", *tag))
                        .cursor_pointer()
                        .child(frame)
                        .on_click(move |evt, window, cx: &mut App| (preview)(evt, window, cx))
                        .into_any_element()
                } else {
                    frame.into_any_element()
                });
            } else {
                body = body.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .size(px(44.))
                                .flex_shrink_0()
                                .rounded_md()
                                .bg(theme.background)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.muted_foreground)
                                .child(Icon::empty().path(icon.clone()).large()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .min_w(px(0.))
                                .child(div().text_sm().truncate().child(name.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(size.clone()),
                                ),
                        ),
                );
            }
            // caption line: name for thumbs, locked pill when set
            let mut meta = div().flex().flex_row().items_center().gap_2();
            if thumb.is_some() {
                meta = meta.child(div().flex_1().text_sm().truncate().child(name.clone()));
            }
            if *locked {
                meta = meta.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(ACCENT_GREEN))
                        .px_2()
                        .py(px(1.))
                        .child(
                            Icon::empty()
                                .path("icons/lock.svg")
                                .size(px(11.))
                                .text_color(rgb(ACCENT_GREEN)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(ACCENT_GREEN))
                                .child("Locked"),
                        ),
                );
            } else {
                meta = meta.child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(size.clone()),
                );
            }
            body = body.child(meta);
            // open action (non-images only; thumbs open via preview)
            if thumb.is_none()
                && let Some(open) = on_open.clone()
            {
                body = body.child(
                    div().flex().flex_row().justify_end().w_full().child(
                        Button::new(("bfile-open", *tag))
                            .ghost()
                            .small()
                            .icon(Icon::empty().path("icons/download.svg"))
                            .label("Open")
                            .on_click(move |evt, window, cx: &mut App| (open)(evt, window, cx)),
                    ),
                );
            }
            body.into_any_element()
        }
        BubbleContent::Contact {
            name,
            phone,
            color,
            initials,
        } => div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                div()
                    .size(px(40.))
                    .flex_shrink_0()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(*color))
                    .text_color(rgb(0xffffff))
                    .font_bold()
                    .text_sm()
                    .child(initials.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_sm().font_bold().child(name.clone()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(phone.clone()),
                    ),
            )
            .into_any_element(),
        BubbleContent::Poll {
            question,
            options,
            votes,
            my_vote,
            tag,
            on_vote,
        } => {
            let total: u32 = votes.iter().sum();
            let mut col = div()
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(240.))
                .child(div().text_sm().font_bold().child(question.clone()));
            for (ix, opt) in options.iter().enumerate() {
                let v = votes.get(ix).copied().unwrap_or(0);
                let pct = if total > 0 {
                    (v as f32 / total as f32 * 100.0) as u32
                } else {
                    0
                };
                let mine = *my_vote == Some(ix);
                let row_id = tag.saturating_mul(64).saturating_add(ix.min(63));
                let mut row = div()
                    .id(("poll-opt", row_id))
                    .relative()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .overflow_hidden();
                row = if mine {
                    row.border_color(theme.primary)
                        .bg(theme.primary.opacity(0.12))
                } else {
                    row.border_color(theme.border)
                };
                row = row
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .bottom_0()
                            .w(relative(if total > 0 {
                                v as f32 / total as f32
                            } else {
                                0.0
                            }))
                            .bg(theme.primary.opacity(if mine { 0.25 } else { 0.12 })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .truncate()
                            .text_color(theme.foreground)
                            .child(format!("{}{}", if mine { "● " } else { "" }, opt.clone())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{v} · {pct}%")),
                    );
                if let Some(vote) = on_vote.clone() {
                    row =
                        row.on_click(move |evt, window, cx: &mut App| (vote)(ix, evt, window, cx));
                }
                col = col.child(row);
            }
            col = col.child(div().text_xs().text_color(theme.muted_foreground).child(
                if total == 1 {
                    "1 vote · tap to change".to_string()
                } else {
                    format!("{total} votes · tap to change")
                },
            ));
            col.into_any_element()
        }
        BubbleContent::Event { title, when } => div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                div()
                    .size(px(40.))
                    .flex_shrink_0()
                    .rounded_md()
                    .bg(rgb(0xe3618c))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0xffffff))
                    .child(Icon::empty().path("icons/calendar.svg")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_sm().font_bold().child(title.clone()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(when.clone()),
                    ),
            )
            .into_any_element(),
    }
}

/// Reaction / star chips under a bubble. Empty when nothing to show.
#[derive(IntoElement)]
pub struct ReactionChips {
    style: StyleRefinement,
    reactions: Vec<String>,
    starred: bool,
    mine: bool,
}

impl ReactionChips {
    pub fn new(reactions: Vec<String>, starred: bool, mine: bool) -> Self {
        Self {
            style: StyleRefinement::default(),
            reactions,
            starred,
            mine,
        }
    }
}

impl Styled for ReactionChips {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ReactionChips {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let chip = |text: String| {
            div()
                .rounded_full()
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .px_2()
                .text_sm()
                .child(text)
        };
        let mut row = div()
            .flex()
            .flex_row()
            .gap_1()
            .pt_1()
            .w_full()
            .when(self.mine, |t| t.justify_end())
            .when(!self.mine, |t| t.justify_start());
        let mut any = false;
        for r in &self.reactions {
            any = true;
            row = row.child(chip(r.clone()));
        }
        if self.starred {
            any = true;
            row = row.child(chip("⭐".to_string()));
        }
        div().when(any, |t| t.child(row))
    }
}
