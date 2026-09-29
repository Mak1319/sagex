//! Three-state theme switch: system / dark / light segmented pill.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
};
use std::rc::Rc;

/// Theme selection, in toggle order.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    /// Follow the OS appearance.
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    /// Apply the choice: re-sync for System, pin for Dark/Light.
    pub fn apply(self, window: &mut Window, cx: &mut App) {
        use gpui_component::{Theme, ThemeMode};
        match self {
            ThemeChoice::System => Theme::sync_system_appearance(Some(window), cx),
            ThemeChoice::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
            ThemeChoice::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        }
    }

    pub fn is_system(self) -> bool {
        matches!(self, ThemeChoice::System)
    }
}

/// Controlled segmented control. Parent owns the mode + applies it.
/// Default layout is vertical (fits the 60px chat rail).
#[derive(IntoElement)]
pub struct ThemeToggle {
    style: StyleRefinement,
    mode: ThemeChoice,
    horizontal: bool,
    on_select: Option<super::ThemeHandler>,
}

impl ThemeToggle {
    pub fn new(mode: ThemeChoice) -> Self {
        Self {
            style: StyleRefinement::default(),
            mode,
            horizontal: false,
            on_select: None,
        }
    }

    /// Compact row for the auth chrome (not the chat rail).
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(&ThemeChoice, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn option(
        id: &'static str,
        icon: &'static str,
        tip: &'static str,
        active: bool,
        cx: &App,
        handler: &Option<super::ThemeHandler>,
        choice: ThemeChoice,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut btn = Button::new(id)
            .ghost()
            .small()
            .icon(Icon::empty().path(icon))
            .tooltip(tip);
        if active {
            // Inset look: container radius (14) minus its padding (3) with
            // one extra px of optical tightening. Border-free so active and
            // idle options keep identical metrics (no layout shift).
            btn = btn.bg(theme.background).rounded(px(10.));
        }
        if let Some(handler) = handler.clone() {
            btn = btn.on_click(move |_, window, cx: &mut App| (handler)(&choice, window, cx));
        }
        btn
    }
}

impl Styled for ThemeToggle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ThemeToggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let horizontal = self.horizontal;
        div()
            .flex()
            .when(horizontal, |t| t.flex_row())
            .when(!horizontal, |t| t.flex_col())
            .items_center()
            .gap(px(2.))
            .rounded(px(14.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.muted)
            .p(px(3.))
            .child(Self::option(
                "theme-system",
                "icons/sun-moon.svg",
                "Follow system",
                self.mode == ThemeChoice::System,
                cx,
                &self.on_select,
                ThemeChoice::System,
            ))
            .child(Self::option(
                "theme-dark",
                "icons/moon.svg",
                "Dark",
                self.mode == ThemeChoice::Dark,
                cx,
                &self.on_select,
                ThemeChoice::Dark,
            ))
            .child(Self::option(
                "theme-light",
                "icons/sun.svg",
                "Light",
                self.mode == ThemeChoice::Light,
                cx,
                &self.on_select,
                ThemeChoice::Light,
            ))
    }
}
