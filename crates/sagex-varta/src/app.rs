use crate::components::title_bar::SysTitleBar;
use crate::components::{canvas::Canvas, icon_rail::IconRail, side_panel::SidePanel};
use crate::theme;
use gpui_kit::base::resizable_panel;
use gpui_kit::component::h_resizable;
use gpui_kit::*;

pub struct App {
    rail: Entity<IconRail>,
    side: Entity<SidePanel>,
    canvas: Entity<Canvas>,
}

impl App {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            rail: cx.new(|_| IconRail::new()),
            side: cx.new(|cx| SidePanel::new(window, cx)),
            canvas: cx.new(|cx| Canvas::new(window, cx)),
        }
    }
}

impl Render for App {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx);
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.canvas)
            .text_color(t.on_surface)
            .child(SysTitleBar::new("Varta"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .w_full()
                    .size_full()
                    .child(self.rail.clone())
                    .child(
                        div().size_full().child(
                            h_resizable("my-layout")
                                .child(resizable_panel().child(self.side.clone()))
                                .child(resizable_panel().child(self.canvas.clone())),
                        ),
                    ),
            )
    }
}
