use gpui_kit::{base::TextView, prelude::*, *};

use crate::{
    components::shared::{IconBtn, KitIcon, MetaChip, NodeAvatar},
    theme::{self, TintColor},
};

/// Static message content for the stream (mock data until send exists).
#[derive(Clone)]
pub struct MessageData {
    pub avatar: char,
    pub tint: TintColor,
    pub name: &'static str,
    pub name_primary: bool,
    pub time: &'static str,
    pub chip: Option<(&'static str, TintColor)>,
    pub loc: &'static str,
    pub body_md: &'static str,
    pub file: Option<FileData>,
}

/// Static file attachment content.
#[derive(Clone)]
pub struct FileData {
    pub name: &'static str,
    pub meta: &'static str,
    pub icon: &'static str,
}

/// Basic text chat card: avatar + name/time/chip/LOC header, rendered
/// markdown body, static hover-style action bar.
#[derive(IntoElement, Clone)]
pub struct MessageCard {
    index: usize,
    data: MessageData,
}

impl MessageCard {
    pub fn new(index: usize, data: MessageData) -> Self {
        Self { index, data }
    }
}

impl RenderOnce for MessageCard {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        let letter: SharedString = self.data.avatar.to_string().into();
        div()
            .relative()
            .flex_none()
            .flex()
            .max_w_full()
            .flex_row()
            .items_start()
            .gap(px(10.0))
            .p(px(8.0))
            .bg(t.card_wash)
            .border_1()
            .border_color(t.card_wash)
            .rounded(px(t.radius))
            .child(NodeAvatar::new(letter, self.data.tint))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .flex_col()
                    .min_w_0()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if self.data.name_primary {
                                        t.primary
                                    } else {
                                        t.on_surface
                                    })
                                    .child(self.data.name),
                            )
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .text_color(t.muted)
                                    .child(self.data.time),
                            )
                            .when_some(self.data.chip, |this, (label, tint)| {
                                this.child(MetaChip::new(label, tint))
                            })
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .text_color(t.muted)
                                    .child(self.data.loc),
                            ),
                    )
                    .child(
                        TextView::markdown(
                            SharedString::from(format!("msg-body-{}", self.index)),
                            self.data.body_md,
                        )
                        .w_full()
                        .text_size(px(13.0))
                        .text_color(t.on_surface),
                    )
                    .when_some(self.data.file, |this, file| this.child(FileCard::new(file))),
            )
            .child(
                // Static action bar (reactions + reply + more).
                div()
                    .absolute()
                    .right(px(8.0))
                    .top(px(8.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .px(px(4.0))
                    .py(px(2.0))
                    .bg(t.lowest)
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(t.radius_sm))
                    .text_color(t.muted)
                    .child(div().text_size(px(11.0)).child("👍"))
                    .child(div().text_size(px(11.0)).child("🚀"))
                    .child(div().flex_none().w(px(1.0)).h(px(12.0)).bg(t.line))
                    .child(KitIcon::new("icons/reply.svg", 13.0).color(t.muted))
                    .child(KitIcon::new("icons/more-horiz.svg", 13.0).color(t.muted)),
            )
    }
}

/// File attachment card inside a message.
#[derive(IntoElement, Clone)]
pub struct FileCard {
    file: FileData,
}

impl FileCard {
    pub fn new(file: FileData) -> Self {
        Self { file }
    }
}

impl RenderOnce for FileCard {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        let blue = t.tint(TintColor::Blue);
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .p(px(8.0))
            .bg(t.lowest)
            .border_1()
            .border_color(t.line)
            .rounded(px(t.radius))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .min_w_0()
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(36.0))
                            .h(px(36.0))
                            .bg(blue.bg)
                            .border_1()
                            .border_color(blue.border)
                            .rounded(px(t.radius_sm))
                            .child(KitIcon::new(self.file.icon, 18.0).color(t.primary)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.on_surface)
                                    .child(div().truncate().child(self.file.name)),
                            )
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .text_color(t.muted)
                                    .child(self.file.meta),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .pl(px(8.0))
                    .child(IconBtn::new("icons/download.svg", 14.0, 24.0))
                    .child(IconBtn::new("icons/more-vert.svg", 14.0, 24.0)),
            )
    }
}
