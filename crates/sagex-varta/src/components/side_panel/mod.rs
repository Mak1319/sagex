use gpui_kit::*;

use crate::{
    components::chat_list::list::ChatList,
    theme,
};

pub struct SidePanel {
    list: Entity<ChatList>,
}

impl SidePanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            list: cx.new(|cx| ChatList::new(window, cx)),
        }
    }
}

impl Render for SidePanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx);
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(t.lowest)
            .border_r_1()
            .border_color(t.line)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .flex_col()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(self.list.clone()),
            )
            .child(
                // Channel footer specs bar.
                div()
                    .flex()
                    .flex_none()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .p(px(8.0))
                    .bg(t.inset)
                    .border_t_1()
                    .border_color(t.line)
                    .text_size(px(9.0))
                    .text_color(t.muted)
                    .child("RECEPTOR: ONLINE")
                    .child(
                        div()
                            .text_color(t.primary)
                            .font_weight(FontWeight::BOLD)
                            .child("PORT: 8080 // WS"),
                    ),
            )
    }
}
