//! Initials avatar circle.

use gpui::{
    App, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, StyleRefinement, Styled,
    Window, div, px,
};
use gpui_component::{ActiveTheme, Sizable, Size, StyledExt as _};

/// Circle with up to 2 initials. Controlled: all visuals come from props.
#[derive(IntoElement)]
pub struct Avatar {
    style: StyleRefinement,
    initials: SharedString,
    color: Option<u32>,
    size: Size,
    text_size: Option<Pixels>,
}

impl Avatar {
    pub fn new(initials: impl Into<SharedString>) -> Self {
        Self {
            style: StyleRefinement::default(),
            initials: initials.into(),
            color: None,
            size: Size::Medium,
            text_size: None,
        }
    }

    /// Background color as `0xRRGGBB`. Defaults to the theme primary.
    pub fn color(mut self, color: u32) -> Self {
        self.color = Some(color);
        self
    }

    /// Explicit diameter, e.g. `px(28.)`.
    pub fn diameter(mut self, diameter: Pixels) -> Self {
        self.style.size.width = Some(diameter.into());
        self.style.size.height = Some(diameter.into());
        self
    }

    /// Explicit glyph size, e.g. `text_xs` scale via `px(11.)`.
    pub fn glyph(mut self, size: Pixels) -> Self {
        self.text_size = Some(size);
        self
    }
}

impl Sizable for Avatar {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for Avatar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let bg: gpui::Hsla = self
            .color
            .map(|c| gpui::rgb(c).into())
            .unwrap_or(theme.primary);
        let (diameter, glyph) = match self.size {
            Size::XSmall => (px(20.), px(9.)),
            Size::Small => (px(28.), px(11.)),
            Size::Medium => (px(40.), px(13.)),
            Size::Large => (px(56.), px(16.)),
            Size::Size(v) => (v, v * 0.34),
        };
        let glyph = self.text_size.unwrap_or(glyph);
        // Apply caller refinements FIRST: assignment replaces the whole
        // style, so visuals chained afterwards are never wiped.
        let mut base = div();
        *base.style() = self.style.clone();
        base.flex_shrink_0()
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(bg)
            .text_color(gpui::rgb(0xffffff))
            .font_bold()
            .size(diameter)
            .text_size(glyph)
            .child(div().text_size(glyph).child(self.initials))
    }
}
