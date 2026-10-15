//! Blueprint Precision button: custom component built on `gpui-base` behavior.
//!
//! `gpui_kit::base::Button` owns interaction, focus, keyboard and
//! accessibility. All presentation (colors, borders, radius, hover/active
//! states) is applied here from `crate::theme` via GPUI's `Styled` API.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BlueprintVariant {
    Primary,
    #[default]
    Ghost,
}

/// Blueprint-styled button. Construct with `BlueprintButton::new(id)`,
/// then `.label(..)`, `.primary()` / `.ghost()`, `.on_click(..)`.
#[derive(IntoElement)]
pub struct BlueprintButton {
    id: SharedString,
    label: SharedString,
    variant: BlueprintVariant,
    on_click: Option<ClickHandler>,
}

impl BlueprintButton {
    pub fn new(id: impl Into<SharedString>) -> Self {
        let id = id.into();
        Self {
            label: id.clone(),
            id,
            variant: BlueprintVariant::default(),
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn primary(mut self) -> Self {
        self.variant = BlueprintVariant::Primary;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = BlueprintVariant::Ghost;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for BlueprintButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg, fg, border, hover_bg) = match self.variant {
            BlueprintVariant::Primary => (
                theme::rgb(theme::colors::PRIMARY_CONTAINER),
                theme::rgb(theme::colors::ON_PRIMARY),
                theme::rgb(theme::colors::PRIMARY_DARK),
                theme::rgb(theme::colors::PRIMARY_DARK),
            ),
            BlueprintVariant::Ghost => (
                theme::rgb(theme::colors::SURFACE_LOWEST),
                theme::rgb(theme::colors::INK),
                theme::rgb(theme::colors::GRID_L2),
                theme::rgb(theme::colors::INSET),
            ),
        };

        let mut button = BaseButton::new(ElementId::Name(self.id))
            .px_3()
            .py_2()
            .rounded(px(theme::radius::DEFAULT))
            .bg(bg)
            .text_color(fg)
            .border_1()
            .border_color(border)
            .text_size(px(13.0))
            .hover(|s| s.bg(hover_bg))
            .child(self.label);
        if let Some(handler) = self.on_click {
            button = button.on_click(move |event, window, cx| handler(event, window, cx));
        }
        button
    }
}
