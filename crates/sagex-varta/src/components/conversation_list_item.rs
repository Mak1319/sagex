//! Conversation list row — manual `gpui-base` expansion only.
//!
//! One variant struct covers the row states via props: `clip` (paperclip
//! after preview text), `pinned` (corner pin), `critical` (red preview
//! text + red dot), unread badge, online dot, active wash.
//! The row is `w_full` and fills whatever width the parent gives it.
//! Hover/active washes come from the shared rail tokens. The right-side meta
//! cluster floats on a scrim of the same row color — time top-right,
//! unread / dot / menu bottom-right — so text slides underneath seamlessly.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::components::avatar::Avatar;
use crate::theme;

type MenuHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const CLIP: &str = "icons/paper-clip.svg";
const PIN: &str = "icons/pin.svg";
const KEBAB: &str = "icons/ellipsis-vertical.svg";

/// Single conversation row. Build with `new(id)` then the property builders.
#[derive(IntoElement)]
//(!) ConversationListItem
pub struct ConversationListItem {
    id: SharedString,
    initial: SharedString,
    avatar_bg: u32,
    avatar_border: u32,
    avatar_fg: u32,
    title: SharedString,
    preview: SharedString,
    time: SharedString,
    unread: usize,
    active: bool,
    online: bool,
    clip: bool,
    pinned: bool,
    critical: bool,
    on_menu: Option<MenuHandler>,
}

impl ConversationListItem {
    pub fn new(id: impl Into<SharedString>) -> Self {
        let id = id.into();
        Self {
            initial: SharedString::from("?"),
            avatar_bg: theme::colors::SURFACE_CONTAINER,
            avatar_border: theme::colors::GRID_L1,
            avatar_fg: theme::colors::INK,
            title: id.clone(),
            preview: SharedString::from(""),
            time: SharedString::from(""),
            unread: 0,
            active: false,
            online: false,
            clip: false,
            pinned: false,
            critical: false,
            on_menu: None,
            id,
        }
    }

    pub fn initial(mut self, initial: impl Into<SharedString>) -> Self {
        self.initial = initial.into();
        self
    }

    pub fn avatar(mut self, bg: u32, border: u32, fg: u32) -> Self {
        self.avatar_bg = bg;
        self.avatar_border = border;
        self.avatar_fg = fg;
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn preview(mut self, preview: impl Into<SharedString>) -> Self {
        self.preview = preview.into();
        self
    }

    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = time.into();
        self
    }

    pub fn unread(mut self, count: usize) -> Self {
        self.unread = count;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn online(mut self, online: bool) -> Self {
        self.online = online;
        self
    }

    pub fn clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    pub fn critical(mut self, critical: bool) -> Self {
        self.critical = critical;
        self
    }

    pub fn on_menu(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_menu = Some(Rc::new(handler));
        self
    }

    pub fn reader_unread(&self) -> usize {
        self.unread
    }
}

impl RenderOnce for ConversationListItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let group = SharedString::from(format!("row-{}", self.id));
        let row_bg = if self.active {
            theme::rgb(theme::colors::ROW_ACTIVE)
        } else {
            theme::rgb(theme::colors::SURFACE_LOWEST)
        };
        let hover_bg = theme::rgb(theme::colors::ROW_HOVER);
        let muted = theme::rgb(theme::colors::MUTED);
        let preview_color = if self.critical {
            theme::rgb(theme::colors::CRITICAL)
        } else {
            muted
        };
        let unread = self.unread;

        let mut kebab = BaseButton::new(ElementId::Name(SharedString::from(format!(
            "{}-menu",
            self.id
        ))))
        .w(px(24.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::radius::DEFAULT))
        .text_color(muted)
        .opacity(0.35)
        .group_hover(group.clone(), |s| s.opacity(1.0));
        if let Some(handler) = self.on_menu {
            kebab = kebab.on_click(move |event, window, cx| handler(event, window, cx));
        }

        div()
            .id(ElementId::Name(self.id.clone()))
            .group(group.clone())
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .w_full()
            .bg(row_bg)
            .hover(move |s| s.bg(hover_bg))
            .border_b_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child({
                let mut face = Avatar::new(self.initial)
                    .bg(self.avatar_bg)
                    .fg(self.avatar_fg)
                    .size(31.0)
                    .border(self.avatar_border);
                if self.online {
                    face = face.presence(theme::colors::SUCCESS);
                }
                face
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme::rgb(theme::colors::INK))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(self.title),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(preview_color)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(self.preview),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .items_end()
                            .py_0()
                            .pl_3()
                            .pr_1()
                            .bg(row_bg)
                            .group_hover(group, |s| s.bg(hover_bg))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .flex_shrink_0()
                                    .when(self.critical, |this| {
                                        this.child(
                                            div()
                                                .w(px(8.0))
                                                .h(px(8.0))
                                                .rounded_full()
                                                .bg(theme::rgb(theme::colors::CRITICAL)),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(muted)
                                            .child(self.time),
                                    )
                                    .when(self.pinned, |this| {
                                        this.child(
                                            svg()
                                                .path(PIN)
                                                .w(px(12.0))
                                                .h(px(12.0))
                                                .flex_shrink_0()
                                                .text_color(theme::rgb(0x004ac6)),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    .gap_1()
                                    .flex_shrink_0()
                                    .when(self.clip, |this| {
                                        this.child(
                                            svg()
                                                .path(CLIP)
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .flex_shrink_0()
                                                .text_color(preview_color),
                                        )
                                    })
                                    .when(unread > 0, |this| {
                                        this.child(
                                            div()
                                                .px_2()
                                                .rounded_full()
                                                .bg(theme::rgb(theme::colors::PRIMARY_CONTAINER))
                                                .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                                                .text_size(px(11.0))
                                                .child(SharedString::from(format!("{unread}"))),
                                        )
                                    })
                                    .child(kebab.justify_center().child(
                                        svg().path(KEBAB).w(px(16.0)).h(px(16.0)).text_color(muted),
                                    )),
                            ),
                    ),
            )
    }
}
