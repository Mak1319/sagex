//! ChatWindow — primary layout: TopBar + IconRail + h_resizable(Channel | ChatCanvas).
//!
//! Manual `gpui-base` expansion only. Resizable panels carry no explicit
//! size or range; the group distributes space and the user drags.

use gpui_kit::base::{ResizableState, h_resizable, resizable_panel};
use gpui_kit::component::{Placement, WindowExt};
use gpui_kit::*;

use crate::layout::channel_panel::ChannelPanel;
use crate::layout::chat_canvas::ChatCanvas;
use crate::layout::conversation_list::ConversationList;
use crate::layout::icon_rail::IconRail;
use crate::layout::inspector::InspectorContent;
use crate::layout::top_bar::TopBar;
use crate::theme;
use gpui_kit::component::input::InputState;

pub struct ChatWindow {
    theme: u8,
    conversations: Entity<ConversationList>,
    layout: Entity<ResizableState>,
    search: Entity<InputState>,
    canvas: Entity<ChatCanvas>,
}

impl ChatWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            theme: 1,
            conversations: cx.new(|_| ConversationList::new()),
            layout: cx.new(|_| ResizableState::default()),
            search: cx.new(|cx| {
                InputState::new(window, cx).placeholder("Search conversations...")
            }),
            canvas: cx.new(|cx| ChatCanvas::new(window, cx)),
        }
    }

    fn set_theme(&mut self, index: u8, cx: &mut Context<Self>) {
        self.theme = index.min(2);
        cx.notify();
    }
}

fn open_inspector(window: &mut Window, cx: &mut App) {
    window.open_sheet_at(Placement::Right, cx, |sheet, _, _| {
        sheet
            .title("Inspector")
            .size(px(360.0))
            .resizable(true)
            .overlay(false)
            .overlay_closable(false)
            .child(InspectorContent::new())
    });
}

impl Render for ChatWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let view = cx.entity().clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::rgb(theme::colors::BACKGROUND))
            .child(
                TopBar::new("Varta — ChatWindow").on_inspector(|_, window, cx| {
                    open_inspector(window, cx);
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        IconRail::new()
                            .theme(theme)
                            .on_theme(move |index, _window, cx: &mut App| {
                                view.update(cx, |view, cx| view.set_theme(index, cx));
                            })
                            .on_settings(|_, window, cx| {
                                open_inspector(window, cx);
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                h_resizable("varta-main")
                                    .with_state(&self.layout)
                                    .child(
                                        resizable_panel()
                                            .size(px(300.0))
                                            .size_range(px(240.0)..Pixels::MAX)
                                            .child(ChannelPanel::new(
                                                self.conversations.clone(),
                                                self.search.clone(),
                                            )),
                                    )
                                    .child(resizable_panel().child(self.canvas.clone())),
                            ),
                    ),
            )
    }
}
