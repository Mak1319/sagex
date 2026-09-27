//! Dropdown menu primitives: rows + capped card.

use gpui::{
    AnyElement, App, ClickEvent, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, Icon};
use std::rc::Rc;

/// One dropdown row: muted glyph + label. Controlled via `on_click`.
#[derive(IntoElement)]
pub struct MenuRow {
    style: StyleRefinement,
    id: (&'static str, usize),
    icon: SharedString,
    label: SharedString,
    trailing: Option<AnyElement>,
    highlighted: bool,
    on_click: Option<super::ClickHandler>,
}

impl MenuRow {
    pub fn new(
        id: (&'static str, usize),
        icon: &'static str,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            style: StyleRefinement::default(),
            id,
            icon: icon.into(),
            label: label.into(),
            trailing: None,
            highlighted: false,
            on_click: None,
        }
    }

    /// Trailing element, e.g. a submenu chevron.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
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

impl Styled for MenuRow {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MenuRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut row = div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .when(self.highlighted, |t| t.bg(theme.muted))
            .hover(|s| s.bg(theme.muted))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        Icon::empty()
                            .path(self.icon)
                            .size(px(16.))
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.foreground)
                            .child(self.label),
                    ),
            );
        if let Some(trailing) = self.trailing {
            row = row.child(trailing);
        }
        if let Some(handler) = self.on_click {
            row = row.on_click(move |evt, window, cx: &mut App| (handler)(evt, window, cx));
        }
        row
    }
}

/// Bordered popover column. Natural height, no scrollbar.
#[derive(IntoElement)]
pub struct MenuCard {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl MenuCard {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl ParentElement for MenuCard {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for MenuCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MenuCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .rounded(px(12.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .py_2()
            .px_2()
            .flex()
            .flex_col()
            .children(self.children)
    }
}
