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
use crate::layout::group_form::{GroupForm, GroupMembersSheet};
use crate::layout::icon_rail::IconRail;
use crate::layout::inspector::InspectorContent;
use crate::layout::single_chat::{SingleChat, SingleSecuritySheet, SingleTarget};
use crate::layout::top_bar::TopBar;
use crate::theme;
use gpui_kit::component::input::InputState;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChatMode {
    Chat,
    NewGroup,
    SingleChat,
}

pub struct ChatWindow {
    theme: u8,
    mode: ChatMode,
    new_menu: bool,
    conversations: Entity<ConversationList>,
    layout: Entity<ResizableState>,
    search: Entity<InputState>,
    canvas: Entity<ChatCanvas>,
    group_form: Entity<GroupForm>,
    single: Entity<SingleChat>,
}

impl ChatWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let win = cx.entity().clone();
        let single_win = win.clone();
        Self {
            theme: 1,
            mode: ChatMode::Chat,
            new_menu: false,
            conversations: cx.new(|_| ConversationList::new()),
            layout: cx.new(|_| ResizableState::default()),
            search: cx.new(|cx| InputState::new(window, cx).placeholder("Search conversations...")),
            canvas: cx.new(|cx| {
                ChatCanvas::new(window, cx).on_new_group(move |_, _, cx: &mut App| {
                    win.update(cx, |w, cx| w.set_mode(ChatMode::NewGroup, cx));
                })
            }),
            group_form: cx.new(|cx| GroupForm::new(window, cx)),
            single: cx.new(|cx| {
                SingleChat::new(window, cx).on_navigate(move |target, _, cx: &mut App| {
                    single_win.update(cx, |w, cx| {
                        w.set_mode(
                            match target {
                                SingleTarget::Chat => ChatMode::Chat,
                                SingleTarget::Group => ChatMode::NewGroup,
                            },
                            cx,
                        )
                    });
                })
            }),
        }
    }

    fn set_theme(&mut self, index: u8, cx: &mut Context<Self>) {
        self.theme = index.min(2);
        cx.notify();
    }

    fn set_mode(&mut self, mode: ChatMode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.new_menu = false;
        cx.notify();
    }

    fn toggle_new_menu(&mut self, cx: &mut Context<Self>) {
        self.new_menu = !self.new_menu;
        cx.notify();
    }
}

fn open_inspector(
    window: &mut Window,
    cx: &mut App,
    mode: ChatMode,
    form: &Entity<GroupForm>,
    single: &Entity<SingleChat>,
) {
    if mode == ChatMode::NewGroup {
        let members = cx.new(|cx| GroupMembersSheet::new(form.clone(), cx));
        window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
            sheet
                .title("Group Members")
                .size(px(360.0))
                .resizable(true)
                .overlay(false)
                .overlay_closable(false)
                .child(members.clone())
        });
        return;
    }
    if mode == ChatMode::SingleChat {
        let security = cx.new(|cx| SingleSecuritySheet::new(single.clone(), cx));
        window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
            sheet
                .title("Conversation Security")
                .size(px(340.0))
                .resizable(true)
                .overlay(false)
                .overlay_closable(false)
                .child(security.clone())
        });
        return;
    }
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
        let mode = self.mode;
        let menu_open = self.new_menu;
        let form = self.group_form.clone();
        let form_rail = self.group_form.clone();
        let new_view = cx.entity().clone();
        let view = cx.entity().clone();
        let title = if mode == ChatMode::Chat {
            "Varta — ChatWindow"
        } else if mode == ChatMode::NewGroup {
            "Varta — New Group"
        } else {
            "Varta — Single Chat"
        };
        let mut topbar = TopBar::new(title);
        if mode != ChatMode::Chat {
            let bv = cx.entity().clone();
            topbar = topbar.on_back(move |_, _, cx: &mut App| {
                bv.update(cx, |w, cx| w.set_mode(ChatMode::Chat, cx));
            });
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::rgb(theme::colors::BACKGROUND))
            .child(topbar.on_inspector({
                let single = self.single.clone();
                move |_, window, cx| {
                    open_inspector(window, cx, mode, &form, &single);
                }
            }))
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
                            .on_settings({
                                let single = self.single.clone();
                                move |_, window, cx| {
                                    open_inspector(window, cx, mode, &form_rail, &single);
                                }
                            }),
                    )
                    .child(
                        div().flex().flex_1().overflow_hidden().child(
                            h_resizable("varta-main")
                                .with_state(&self.layout)
                                .child(
                                    resizable_panel()
                                        .size(px(300.0))
                                        .size_range(px(240.0)..Pixels::MAX)
                                        .child(
                                            ChannelPanel::new(
                                                self.conversations.clone(),
                                                self.search.clone(),
                                            )
                                            .new_menu_open(menu_open)
                                            .on_new_toggle({
                                                let tv = new_view.clone();
                                                move |_, cx: &mut App| {
                                                    tv.update(cx, |w, cx| w.toggle_new_menu(cx));
                                                }
                                            })
                                            .on_new_single({
                                                let sv = new_view.clone();
                                                move |_, cx: &mut App| {
                                                    sv.update(cx, |w, cx| {
                                                        w.set_mode(ChatMode::SingleChat, cx)
                                                    });
                                                }
                                            })
                                            .on_new_group({
                                                let gv = new_view.clone();
                                                move |_, cx: &mut App| {
                                                    gv.update(cx, |w, cx| {
                                                        w.set_mode(ChatMode::NewGroup, cx)
                                                    });
                                                }
                                            }),
                                        ),
                                )
                                .child(resizable_panel().child(if mode == ChatMode::NewGroup {
                                    self.group_form.clone().into_any_element()
                                } else if mode == ChatMode::SingleChat {
                                    self.single.clone().into_any_element()
                                } else {
                                    self.canvas.clone().into_any_element()
                                })),
                        ),
                    ),
            )
    }
}
