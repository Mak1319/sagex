//! Message composer: attach + emoji + borderless input + mic/send pill.

use gpui::{
    App, ClickEvent, Entity, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled,
    Window, div,
};
use gpui_component::{
    Icon,
    button::{Button, ButtonVariants},
    input::Input,
    input::InputState,
};
use std::rc::Rc;

/// Controlled composer. Parent owns the input entity + send logic.
#[derive(IntoElement)]
pub struct MessageComposer {
    style: StyleRefinement,
    composer: Entity<InputState>,
    empty: bool,
    on_attach: Option<super::ClickHandler>,
    on_emoji: Option<super::ClickHandler>,
    on_send: Option<super::ClickHandler>,
}

impl MessageComposer {
    pub fn new(composer: &Entity<InputState>, empty: bool) -> Self {
        Self {
            style: StyleRefinement::default(),
            composer: composer.clone(),
            empty,
            on_attach: None,
            on_emoji: None,
            on_send: None,
        }
    }

    pub fn on_attach(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_attach = Some(Rc::new(handler));
        self
    }

    pub fn on_emoji(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_emoji = Some(Rc::new(handler));
        self
    }

    pub fn on_send(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_send = Some(Rc::new(handler));
        self
    }
}

impl Styled for MessageComposer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MessageComposer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = gpui_component::ActiveTheme::theme(cx).clone();
        let mut attach = Button::new("composer-attach")
            .ghost()
            .icon(Icon::empty().path("icons/paperclip.svg"));
        if let Some(handler) = self.on_attach {
            attach = attach.on_click(move |evt, window, cx: &mut App| (handler)(evt, window, cx));
        }
        let mut emoji = Button::new("composer-emoji")
            .ghost()
            .icon(Icon::empty().path("icons/smile.svg"));
        if let Some(handler) = self.on_emoji {
            emoji = emoji.on_click(move |evt, window, cx: &mut App| (handler)(evt, window, cx));
        }
        let mut send = Button::new("composer-send").primary().icon(if self.empty {
            Icon::empty().path("icons/mic.svg")
        } else {
            Icon::empty().path("icons/send.svg")
        });
        if let Some(handler) = self.on_send {
            send = send.on_click(move |evt, window, cx: &mut App| (handler)(evt, window, cx));
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .bg(theme.background)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .rounded_full()
                    .bg(theme.muted)
                    .px_2()
                    .py_1()
                    .child(attach)
                    .child(emoji)
                    .child(
                        div().flex_1().child(
                            Input::new(&self.composer)
                                .appearance(false)
                                .bordered(false)
                                .cleanable(true)
                                .w_full(),
                        ),
                    )
                    .child(send),
            )
    }
}
