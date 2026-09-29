//! Password prompt popup (no backdrop — the caller places it).
//!
//! Reusable: title, subtitle, placeholder and confirm label are all
//! configurable; the entered password leaves through `on_confirm`.
//! Rendered in theme tokens after the reference mock: danger-tinted badge
//! and field border, ghost Cancel, solid danger confirm.

use gpui::{
    App, ClickEvent, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, Window, div, px,
};
use gpui_component::{ActiveTheme, Icon, Sizable, StyledExt as _, input::Input};
use std::rc::Rc;

/// `password: String` leaves through this handler.
pub type PasswordConfirmHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct PasswordModal {
    style: StyleRefinement,
    input: Entity<gpui_component::input::InputState>,
    title: SharedString,
    subtitle: SharedString,
    placeholder: SharedString,
    confirm_label: SharedString,
    on_confirm: Option<PasswordConfirmHandler>,
    on_cancel: Option<super::ClickHandler>,
}

/// Builder-style config (reusable across future callers).
#[allow(dead_code)]
impl PasswordModal {
    pub fn new(input: &Entity<gpui_component::input::InputState>) -> Self {
        Self {
            style: StyleRefinement::default(),
            input: input.clone(),
            title: "Enter Password".into(),
            subtitle: "Enter your password to unlock and enable controls.".into(),
            placeholder: "Password".into(),
            confirm_label: "Unlock".into(),
            on_confirm: None,
            on_cancel: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn confirm_label(mut self, label: impl Into<SharedString>) -> Self {
        self.confirm_label = label.into();
        self
    }

    pub fn on_confirm(mut self, handler: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }

    pub fn on_cancel(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }
}

impl Styled for PasswordModal {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for PasswordModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let input = self.input.clone();
        // Masked field with eye toggle; placeholder is (re)set each paint
        // like the other single-line inputs in the app.
        input.update(cx, |s, cx| {
            s.set_placeholder(self.placeholder.to_string(), _window, cx)
        });
        let confirm_input = self.input.clone();
        let confirm = self.on_confirm.clone();
        let cancel = self.on_cancel.clone();
        let cancel_x = self.on_cancel.clone();
        div()
            .w_full()
            .max_w(px(540.))
            .relative()
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .id("passwd-x")
                    .absolute()
                    .top(px(12.))
                    .right(px(12.))
                    .size(px(28.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.bg(theme.muted))
                    .child(Icon::empty().path("icons/x.svg").small())
                    .on_click(move |evt, window, cx: &mut App| {
                        if let Some(cancel) = cancel_x.clone() {
                            (cancel)(evt, window, cx);
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_4()
                    .w_full()
                    .child(
                        div()
                            .size(px(56.))
                            .flex_shrink_0()
                            .rounded(px(16.))
                            .bg(theme.danger.opacity(0.14))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme.danger)
                            .child(Icon::empty().path("icons/lock.svg").size(px(26.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_bold()
                                    .text_color(theme.foreground)
                                    .child(self.title.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(self.subtitle.clone()),
                            ),
                    ),
            )
            .child(
                // 48px field shell (single-line Inputs ignore height):
                // danger-tinted border per the mock, key icon + borderless
                // masked input with eye toggle inside.
                div()
                    .w_full()
                    .h(px(48.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(theme.danger)
                    .bg(theme.background)
                    .px_3()
                    .child(
                        Icon::empty()
                            .path("icons/key.svg")
                            .large()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div().flex_1().min_w(px(0.)).child(
                            Input::new(&self.input)
                                .bordered(false)
                                .appearance(false)
                                .mask_toggle()
                                .w_full(),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .w_full()
                    .child(
                        div()
                            .id("passwd-cancel")
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.background)
                            .px_4()
                            .py_2()
                            .cursor_pointer()
                            .text_color(theme.foreground)
                            .child(div().text_sm().child("Cancel"))
                            .on_click(move |evt, window, cx: &mut App| {
                                if let Some(cancel) = cancel.clone() {
                                    (cancel)(evt, window, cx);
                                }
                            }),
                    )
                    .child(
                        div()
                            .id("passwd-confirm")
                            .rounded_md()
                            .bg(theme.danger)
                            .px_4()
                            .py_2()
                            .cursor_pointer()
                            .text_color(theme.danger_foreground)
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .child(self.confirm_label.clone()),
                            )
                            .on_click(move |_, window, cx: &mut App| {
                                if let Some(confirm) = confirm.clone() {
                                    let pw = confirm_input.read(cx).value().to_string();
                                    (confirm)(pw, window, cx);
                                }
                            }),
                    ),
            )
            .into_any_element()
    }
}
