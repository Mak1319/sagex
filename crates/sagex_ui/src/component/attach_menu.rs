//! WhatsApp-style attach sheet: rows map by index in chat code.

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, div, px, rgb,
};
use gpui_component::{ActiveTheme, Icon};
use std::rc::Rc;

/// Controlled sheet. Parent maps the index to an action.
#[derive(IntoElement)]
pub struct AttachMenu {
    style: StyleRefinement,
    on_select: Option<super::IndexHandler>,
}

impl AttachMenu {
    /// (icon path, label, chip color) rows. Indices: 0 Document,
    /// 1 Photos & videos, 2 Audio (record), 3 Poll (create).
    pub const ITEMS: &'static [(&'static str, &'static str, u32)] = &[
        ("icons/file-text.svg", "Document", 0x7f5af0),
        ("icons/image.svg", "Photos & videos", 0x0199d5),
        ("icons/headphones.svg", "Audio", 0xe6890b),
        ("icons/chart-column.svg", "Poll", 0xe8a90b),
    ];

    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            on_select: None,
        }
    }

    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for AttachMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AttachMenu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows = div().flex().flex_col().gap_1().py_2();
        for (ix, (icon, label, color)) in Self::ITEMS.iter().enumerate() {
            let mut row = div()
                .id(("attach-item", ix))
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .px_4()
                .py_2()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme.muted))
                .child(
                    div()
                        .size(px(32.))
                        .flex_shrink_0()
                        .rounded(px(8.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(*color))
                        .text_color(rgb(0xffffff))
                        .child(Icon::empty().path(*icon).size(px(16.))),
                )
                .child(div().text_color(theme.foreground).child(label.to_string()));
            if let Some(handler) = self.on_select.clone() {
                row = row.on_click(move |_, window, cx: &mut App| (handler)(&ix, window, cx));
            }
            rows = rows.child(row);
        }
        div()
            .absolute()
            .bottom(px(64.))
            .left(px(16.))
            .w(px(260.))
            .rounded(px(16.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .child(rows)
    }
}
