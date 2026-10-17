//! Top bar — manual `gpui-base` expansion only (Styled + theme, no component layer).

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

/// Fixed-height top bar, as-is from Stitch header spec.
#[derive(IntoElement)]
pub struct TopBar {
    title: SharedString,
    on_back: Option<BackHandler>,
    on_inspector: Option<InspectorHandler>,
}

impl TopBar {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            on_back: None,
            on_inspector: None,
        }
    }

    pub fn on_back(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_back = Some(std::rc::Rc::new(handler));
        self
    }

    pub fn on_inspector(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_inspector = Some(std::rc::Rc::new(handler));
        self
    }
}

type InspectorHandler = std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

type BackHandler = std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

impl RenderOnce for TopBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let title = self.title;
        let mut inspector_btn = BaseButton::new(ElementId::Name("topbar-inspector".into()))
            .px_2()
            .py_1()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .text_color(theme::rgb(theme::colors::INK))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L2))
            .text_size(px(12.0))
            .child("Inspector");
        if let Some(handler) = self.on_inspector {
            inspector_btn = inspector_btn
                .on_click(move |event, window, cx| handler(event, window, cx));
        }

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(40.0))
            .px_3()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_b_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child({
                        let mut left = div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2();
                        if let Some(handler) = self.on_back {
                            left = left.child(
                                BaseButton::new(ElementId::Name("topbar-back".into()))
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(theme::radius::DEFAULT))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .hover(|s| {
                                        s.bg(theme::rgb(theme::colors::ROW_HOVER))
                                    })
                                    .on_click(move |event, window, cx| {
                                        handler(event, window, cx)
                                    })
                                    .child(
                                        svg()
                                            .path("icons/arrow-left.svg")
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .text_color(theme::rgb(
                                                theme::colors::MUTED,
                                            )),
                                    ),
                            );
                        }
                        left.child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_1()
                                .child(
                                    div().w(px(10.)).h(px(10.)).rounded_full().bg(
                                        theme::rgb(0xfb7185),
                                    ),
                                )
                                .child(
                                    div().w(px(10.)).h(px(10.)).rounded_full().bg(
                                        theme::rgb(0xfbbf24),
                                    ),
                                )
                                .child(
                                    div().w(px(10.)).h(px(10.)).rounded_full().bg(
                                        theme::rgb(0x34d399),
                                    ),
                                ),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::INK))
                            .child(title),
                    ),
            )
            .child(inspector_btn)
    }
}
