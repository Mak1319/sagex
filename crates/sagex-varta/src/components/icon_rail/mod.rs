use gpui_kit::{
    component::{Icon, Selectable as _, Sizable, tab::Tab},
    *,
};

use crate::theme;

/// Vertical icon-rail tabs: (accessible label, icon asset).
///
/// `TabBar` is horizontal-only (row layout + x-axis sliding indicator),
/// so the 48px vertical rail composes individual segmented `Tab`s in a
/// `flex_col` container instead — the same controlled-selection pattern
/// as the `Tabs` primitive docs: the parent owns `selected`, each tab
/// reflects it, and clicks update the parent + `cx.notify()`.
const TABS: [(&str, &str); 2] = [
    ("Chat", "icons/chat-bubble.svg"),
    ("Threads", "icons/threads.svg"),
];

pub struct IconRail {
    selected: usize,
}

impl IconRail {
    pub fn new() -> Self {
        Self { selected: 0 }
    }
}

impl Render for IconRail {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx);
        let entity = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .items_center()
            .w(px(48.0))
            .h_full()
            .py(px(8.0))
            .gap_1()
            .bg(t.canvas)
            .border_r_1()
            .border_color(t.line)
            // AI instruction : do not add logo hare
            .children(TABS.into_iter().enumerate().map(|(index, (label, icon))| {
                let entity = entity.clone();
                // Unique scope per tab: a standalone `Tab` falls back to the
                // same internal id, so siblings need distinct parents to keep
                // click targets unambiguous.
                div()
                    .id(SharedString::from(format!("icon-rail-tab-{index}")))
                    .child(
                        Tab::new()
                            .icon(Icon::empty().path(icon).large())
                            .aria_label(label)
                            .segmented()
                            .selected(self.selected == index)
                            .on_click(move |_, _, cx| {
                                _ = entity.update(cx, |this: &mut IconRail, cx| {
                                    this.selected = index;
                                    cx.notify();
                                });
                            }),
                    )
            }))
    }
}
