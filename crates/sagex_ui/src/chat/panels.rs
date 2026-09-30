//! Account panels: profile (+logout) and new chat (user search → DM/group).
//! Rendered full-bleed in the main column (same pattern as group info).

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Sizable, StyledExt as _,
    button::{Button, ButtonVariants},
    input::Input,
    label::Label,
    v_flex,
};

use super::ChatApp;

fn card() -> gpui::Div {
    div()
        .w_full()
        .max_w(px(480.))
        .flex()
        .flex_col()
        .gap_4()
        .p_6()
        .rounded_lg()
        .border_1()
}

impl ChatApp {
    pub(super) fn render_profile(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let me = self.me.clone().unwrap_or_else(|| crate::backend::UserDto {
            id: String::new(),
            email: "…".into(),
            username: "…".into(),
            display_name: None,
            status: None,
            avatar_key: None,
            is_verified: false,
            created_at: String::new(),
        });
        let days_left = self
            .sess
            .as_ref()
            .map(|s| (s.refresh_expires_at - crate::backend::now_unix()).max(0) / 86_400)
            .unwrap_or(0);
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
                card()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(
                        div()
                            .text_lg()
                            .font_bold()
                            .text_color(theme.foreground)
                            .child("Profile"),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Label::new(format!("Email: {}", me.email)).text_sm())
                            .child(Label::new(format!("Username: @{}", me.username)).text_sm())
                            .child(
                                Label::new(format!(
                                    "Session refresh valid ~{days_left} more day(s) · {}",
                                    self.api.base()
                                ))
                                .text_sm(),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new("Display name").text_sm())
                            .child(Input::new(&self.profile_name).cleanable(true).w_full()),
                    )
                    .child(
                        Button::new("profile-save")
                            .primary()
                            .label("Save display name")
                            .w_full()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_profile(cx);
                            })),
                    )
                    .child(
                        Button::new("profile-logout")
                            .label("Log out this device")
                            .w_full()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.do_logout(false, cx);
                            })),
                    )
                    .child(
                        Button::new("profile-logout-all")
                            .danger()
                            .label("Log out everywhere")
                            .w_full()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.do_logout(true, cx);
                            })),
                    )
                    .child(
                        Button::new("profile-back")
                            .link()
                            .small()
                            .label("Back to chats")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_profile = false;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_new_chat(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let results = self.user_results.clone();
        let selected = self.new_members.clone();
        let is_dm = self.new_is_dm;
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
                card()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(
                        div()
                            .text_lg()
                            .font_bold()
                            .text_color(theme.foreground)
                            .child("New chat"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                Button::new("nc-dm")
                                    .small()
                                    .label("Direct")
                                    .when(!is_dm, |b| b.ghost())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.new_is_dm = true;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("nc-group")
                                    .small()
                                    .label("Group")
                                    .when(is_dm, |b| b.ghost())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.new_is_dm = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when(!is_dm, |t| {
                        t.child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Group name").text_sm())
                                .child(Input::new(&self.room_name).cleanable(true).w_full()),
                        )
                    })
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new("Find people (username or email)").text_sm())
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(div().flex_1().child(
                                        Input::new(&self.user_search).cleanable(true).w_full(),
                                    ))
                                    .child(
                                        Button::new("nc-search")
                                            .primary()
                                            .label("Search")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.search_users(cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .children(results.into_iter().enumerate().map(|(ix, u)| {
                                let uid = u.id.clone();
                                let label = u
                                    .display_name
                                    .clone()
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or_else(|| u.username.clone());
                                let mark = if selected.contains(&uid) { "✓ " } else { "" };
                                div()
                                    .id(("nc-user", ix))
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.muted))
                                    .text_color(theme.foreground)
                                    .text_sm()
                                    .child(format!("{mark}{label} (@{})", u.username))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.pick_user(uid.clone(), cx);
                                    }))
                                    .into_any_element()
                            })),
                    )
                    .when(!is_dm, |t| {
                        t.child(
                            Button::new("nc-create")
                                .primary()
                                .label(format!("Create group ({} selected)", selected.len()))
                                .w_full()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.create_group(cx);
                                })),
                        )
                    })
                    .child(
                        Button::new("nc-cancel")
                            .link()
                            .small()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_new_chat = false;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
