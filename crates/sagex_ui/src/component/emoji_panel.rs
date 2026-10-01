//! WhatsApp-style emoji panel: category tabs, search, grid, GIF tab.
//!
//! Controlled: search entity + tab/gif state live in the parent.
//! Anchored to the composer column (full width of the chat pane).

use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, ScrollHandle,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
    rgb,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    input::InputState,
};
use std::rc::Rc;

/// One category: label, tab glyph, emojis.
pub type EmojiCategory = (&'static str, &'static str, &'static [&'static str]);

/// Controlled panel. Search/tab/gif state lives in the parent.
#[derive(IntoElement)]
pub struct EmojiPanel {
    style: StyleRefinement,
    search: Entity<InputState>,
    scroll: ScrollHandle,
    categories: &'static [EmojiCategory],
    aliases: &'static [(&'static str, &'static str)],
    tab: usize,
    show_gif: bool,
    on_tab: Option<super::IndexHandler>,
    on_pick: Option<super::TextHandler>,
    on_gif: Option<super::FlagHandler>,
}

impl EmojiPanel {
    pub fn new(
        search: &Entity<InputState>,
        scroll: &ScrollHandle,
        categories: &'static [EmojiCategory],
        aliases: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            style: StyleRefinement::default(),
            search: search.clone(),
            scroll: scroll.clone(),
            categories,
            aliases,
            tab: 0,
            show_gif: false,
            on_tab: None,
            on_pick: None,
            on_gif: None,
        }
    }

    pub fn tab(mut self, tab: usize) -> Self {
        self.tab = tab;
        self
    }

    pub fn show_gif(mut self, show: bool) -> Self {
        self.show_gif = show;
        self
    }

    pub fn on_tab(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_tab = Some(Rc::new(handler));
        self
    }

    pub fn on_pick(mut self, handler: impl Fn(&String, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }

    pub fn on_gif(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_gif = Some(Rc::new(handler));
        self
    }
}

impl Styled for EmojiPanel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for EmojiPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tab = self.tab.min(self.categories.len().saturating_sub(1));
        let q = self.search.read(cx).value().trim().to_lowercase();

        let mut tabs = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_1()
            .w_full();
        for (ix, (_, glyph, _)) in self.categories.iter().enumerate() {
            let active = !self.show_gif && tab == ix;
            let handler = self.on_tab.clone();
            tabs = tabs.child(
                div()
                    .id(("emoji-tab-btn", ix))
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .py(px(8.))
                    .text_lg()
                    .cursor_pointer()
                    .border_b_2()
                    .when(active, |t| {
                        t.border_color(rgb(super::colors::ACCENT_GREEN))
                            .text_color(theme.foreground)
                    })
                    .when(!active, |t| {
                        t.border_color(gpui::transparent_black())
                            .text_color(theme.muted_foreground)
                    })
                    .on_click(move |_, window, cx: &mut App| {
                        if let Some(handler) = handler.clone() {
                            (handler)(&ix, window, cx)
                        }
                    })
                    .child(glyph.to_string()),
            );
        }

        let mut hits: Vec<String> = vec![];
        if !self.show_gif {
            if q.is_empty() {
                hits = self.categories[tab]
                    .2
                    .iter()
                    .map(|e| e.to_string())
                    .collect();
            } else {
                for (glyph, keys) in self.aliases {
                    if keys.contains(&q) {
                        hits.push(glyph.to_string());
                    }
                }
                for e in self.categories[tab].2.iter().filter(|e| e.contains(&q)) {
                    hits.push(e.to_string());
                }
            }
        }
        let mut cells: Vec<gpui::AnyElement> = vec![];
        for (cix, glyph) in hits.into_iter().enumerate() {
            let label = glyph.clone();
            cells.push(
                div()
                    .id(("emoji-cell", cix))
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_lg()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.muted))
                    .on_click({
                        let handler = self.on_pick.clone();
                        move |_, window, cx: &mut App| {
                            if let Some(handler) = handler.clone() {
                                (handler)(&label, window, cx)
                            }
                        }
                    })
                    .child(glyph)
                    .into_any_element(),
            );
        }

        let (section_label, grid): (String, gpui::AnyElement) = if self.show_gif {
            (
                "GIFs".to_string(),
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(248.))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("GIF search (UI preview)")
                    .into_any_element(),
            )
        } else {
            (
                self.categories[tab].0.to_string(),
                div()
                    .id("emoji-grid")
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(2.))
                    .h(px(248.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(cells)
                    .into_any_element(),
            )
        };

        let gif_handler = self.on_gif.clone();
        div()
            .absolute()
            .bottom(px(56.))
            .left(px(8.))
            .right(px(8.))
            .rounded(px(12.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(tabs)
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .child(Input::new(&self.search).cleanable(true).w_full()),
            )
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(section_label),
            )
            .child(div().px_2().pt_1().child(grid))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_around()
                    .mt_1()
                    .px_2()
                    .py_1()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(bottom_btn(
                        "emoji-bottom-emoji",
                        "🙂 Emoji",
                        false,
                        !self.show_gif,
                        gif_handler.clone(),
                    ))
                    .child(bottom_btn(
                        "emoji-bottom-gif",
                        "GIF",
                        true,
                        self.show_gif,
                        gif_handler.clone(),
                    ))
                    .child(bottom_btn(
                        "emoji-bottom-sticker",
                        "Stickers",
                        true,
                        false,
                        gif_handler,
                    )),
            )
    }
}

fn bottom_btn(
    id: &'static str,
    label: &str,
    gif: bool,
    active: bool,
    handler: Option<super::FlagHandler>,
) -> impl IntoElement {
    let mut btn = Button::new(id).ghost().small().label(label.to_string());
    if active {
        btn = btn.primary();
    }
    if let Some(handler) = handler {
        btn = btn.on_click(move |_, window, cx: &mut App| (handler)(&gif, window, cx));
    }
    btn
}
