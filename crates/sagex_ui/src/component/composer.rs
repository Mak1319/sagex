//! Message composer: attach + emoji + borderless input + mic/send pill.

use gpui::{
    App, ClickEvent, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, StyleRefinement, Styled, Window, div, px,
};
use gpui_component::{
    Icon,
    button::{Button, ButtonVariants},
    input::Input,
    input::InputState,
};
use std::rc::Rc;

/// Press-and-hold mic handler pair (WhatsApp-style recording).
pub type MicHoldHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Controlled composer. Parent owns the input entity + send logic.
#[derive(IntoElement)]
pub struct MessageComposer {
    style: StyleRefinement,
    composer: Entity<InputState>,
    empty: bool,
    on_attach: Option<super::ClickHandler>,
    on_emoji: Option<super::ClickHandler>,
    on_send: Option<super::ClickHandler>,
    on_mic_down: Option<MicHoldHandler>,
    on_mic_up: Option<MicHoldHandler>,
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
            on_mic_down: None,
            on_mic_up: None,
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

    /// Press handler for the mic pill (empty composer). Pair with
    /// [`Self::on_mic_up`]; a quick tap resolves to locked recording.
    pub fn on_mic_down(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_mic_down = Some(Rc::new(handler));
        self
    }

    /// Release handler for the mic pill (empty composer).
    pub fn on_mic_up(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_mic_up = Some(Rc::new(handler));
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
        // Hold-to-record mic: when the composer is empty and hold handlers
        // are wired, the mic pill tracks press/release instead of clicking.
        let send = if self.empty && (self.on_mic_down.is_some() || self.on_mic_up.is_some()) {
            let mut mic = div()
                .id("composer-mic")
                .size(px(36.))
                .flex_shrink_0()
                .rounded_full()
                .bg(theme.primary)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(theme.primary_foreground)
                .child(Icon::empty().path("icons/mic.svg"));
            if let Some(down) = self.on_mic_down {
                mic = mic.on_mouse_down(MouseButton::Left, move |_, window, cx: &mut App| {
                    (down)(window, cx)
                });
            }
            if let Some(up) = self.on_mic_up {
                mic = mic.on_mouse_up(MouseButton::Left, move |_, window, cx: &mut App| {
                    (up)(window, cx)
                });
            }
            mic.into_any_element()
        } else {
            send.into_any_element()
        };
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
