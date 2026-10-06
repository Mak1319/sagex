use std::rc::Rc;

use gpui_kit::{prelude::*, *};

use crate::{
    components::shared::{KitIcon, NodeAvatar},
    theme::{self, TintColor},
};

/// Static row content. Pinned chats sort first; section headers are
/// intentionally absent — one list, pin flag only.
#[derive(Clone)]
pub struct ChatData {
    pub label: &'static str,
    pub preview: &'static str,
    pub time: &'static str,
    pub avatar: char,
    pub tint: TintColor,
    pub pinned: bool,
    pub unread: u32,
    pub alert: bool,
    pub muted: bool,
    pub attachment: bool,
    pub online: bool,
}

/// One chat row. State flags compose freely: any combination of
/// selected / unread / alert / muted / pinned / attachment renders —
/// each flag paints its own independent layer.
#[derive(IntoElement, Clone)]
pub struct ChatRow {
    data: ChatData,
    index: usize,
    selected: bool,
    on_select: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl ChatRow {
    pub fn new(index: usize, data: ChatData) -> Self {
        Self {
            index,
            data,
            selected: false,
            on_select: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ChatRow {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        let blue = t.tint(TintColor::Blue);
        let row_bg = if self.selected { blue.bg } else { t.lowest };
        let label_color = if self.selected {
            t.primary
        } else {
            t.on_surface
        };
        let preview_color = if self.data.alert {
            t.critical
        } else if self.data.muted {
            t.muted
        } else {
            t.on_variant
        };
        let time_color = if self.selected { t.primary } else { t.muted };

        let letter: SharedString = self.data.avatar.to_string().into();
        let avatar = if self.data.online {
            NodeAvatar::new(letter, self.data.tint).presence()
        } else {
            NodeAvatar::new(letter, self.data.tint)
        };

        // Bottom slot of the meta overlay: exactly one of pin / unread /
        // alert dot / attachment / nothing.
        let suffix: AnyElement = if self.data.pinned {
            KitIcon::new("icons/push-pin.svg", 12.0)
                .color(t.muted)
                .into_any_element()
        } else if self.data.unread > 0 {
            div()
                .px(px(6.0))
                .rounded(px(t.radius_sm))
                .bg(t.primary)
                .text_color(t.on_primary)
                .border_1()
                .border_color(t.primary_container)
                .text_size(px(9.0))
                .font_weight(FontWeight::BOLD)
                .child(SharedString::from(self.data.unread.to_string()))
                .into_any_element()
        } else if self.data.alert {
            div()
                .w(px(6.0))
                .h(px(6.0))
                .rounded_full()
                .bg(t.critical)
                .into_any_element()
        } else if self.data.attachment {
            KitIcon::new("icons/attachment.svg", 12.0)
                .color(t.muted)
                .into_any_element()
        } else {
            div().into_any_element()
        };

        let on_select = self.on_select.clone();
        div()
            // `.id()` upgrades the div to stateful: required before
            // `.on_click` in this GPUI version. It also scopes the row's
            // interactive state so siblings never collide.
            .id(SharedString::from(format!("chat-row-{}", self.index)))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(8.0))
            .flex_none()
            .cursor_pointer()
            .bg(row_bg)
            .border_l_2()
            .border_color(if self.selected { t.primary } else { row_bg })
            .when_some(on_select, |this, f| {
                this.on_click(move |_, window, cx| f(window, cx))
            })
            .child(avatar)
            // Text block reserves room for the meta overlay; when the panel
            // narrows past that reserve the text slides UNDER the opaque
            // overlay instead of squeezing it.
            .child(
                div()
                    .flex()
                    .flex_1()
                    .flex_col()
                    .min_w_0()
                    .pr(px(64.0))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .text_size(px(12.5))
                            .font_weight(if self.selected {
                                FontWeight::BOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(label_color)
                            .child(div().truncate().child(SharedString::from(self.data.label))),
                    )
                    .child(
                        div().text_size(px(11.0)).text_color(preview_color).child(
                            div()
                                .truncate()
                                .child(SharedString::from(self.data.preview)),
                        ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(12.0))
                    .top_0()
                    .bottom_0()
                    .py(px(8.0))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .items_end()
                    .bg(row_bg)
                    .pl(px(8.0))
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(time_color)
                            .font_weight(if self.selected {
                                FontWeight::BOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(SharedString::from(self.data.time)),
                    )
                    .child(suffix),
            )
    }
}
