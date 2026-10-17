//! Single-chat setup — layout for the ChatWindow canvas slot.
//!
//! Individual counterpart to the group form: recipient lookup with a
//! small static directory, conversation-security controls, and navigation
//! back to chat or over to the group form. All controls live; no backend.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::WindowExt;
use gpui_kit::*;

use crate::components::mono_link::MonoLink;
use crate::routes::profile::ProfilePage;
use crate::theme;

/// Navigation targets out of the single-chat surface.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SingleTarget {
    Chat,
    Group,
}

type NavigateHandler = Rc<dyn Fn(SingleTarget, &mut Window, &mut App)>;

struct Peer {
    name: &'static str,
    handle: &'static str,
    pronouns: &'static str,
    bio: &'static str,
    mutuals: &'static str,
    presence: u32,
    online: bool,
}

const DIRECTORY: &[Peer] = &[
    Peer {
        name: "Daria Kova",
        handle: "@dariakova",
        pronouns: "she/her",
        bio: "Design engineer and 3D precision enthusiast. Building CAD pipelines.",
        mutuals: "Shared 2 mutual contacts · Connected since October 2024",
        presence: theme::colors::SUCCESS,
        online: true,
    },
    Peer {
        name: "Julian Thorne",
        handle: "@jthorne",
        pronouns: "he/him",
        bio: "GPU profiling nerd. Shader reviews on request.",
        mutuals: "Shared 5 mutual contacts · Connected since March 2024",
        presence: theme::colors::SUCCESS,
        online: true,
    },
    Peer {
        name: "Anya Lindqvist",
        handle: "@anyalind",
        pronouns: "she/her",
        bio: "Realtime graphics and WGSL tooling.",
        mutuals: "Shared 1 mutual contact · Connected since January 2025",
        presence: theme::colors::MUTED,
        online: false,
    },
];

const TIMERS: &[&str] = &[
    "Off (Retain chat history)",
    "24 hours",
    "7 days",
];

pub struct SingleChat {
    lookup: Entity<InputState>,
    found: Option<usize>,
    searched: bool,
    disappearing: usize,
    receipts: bool,
    autodl: bool,
    error: Option<SharedString>,
    on_navigate: Option<NavigateHandler>,
    _watches: Vec<Subscription>,
}

impl SingleChat {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let lookup =
            cx.new(|cx| InputState::new(window, cx).placeholder("@dariakova"));
        let mut _watches = vec![cx.observe(&lookup, |view: &mut Self, _, cx| {
            view.error = None;
            cx.notify();
        })];
        _watches.push(cx.subscribe(
            &lookup,
            |view: &mut Self, _: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    view.find(cx);
                }
            },
        ));
        Self {
            lookup,
            found: None,
            searched: false,
            disappearing: 0,
            receipts: true,
            autodl: true,
            error: None,
            on_navigate: None,
            _watches,
        }
    }

    pub fn on_navigate(
        mut self,
        handler: impl Fn(SingleTarget, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_navigate = Some(Rc::new(handler));
        self
    }

    fn go(&self, target: SingleTarget, window: &mut Window, cx: &mut App) {
        if let Some(nav) = self.on_navigate.clone() {
            nav(target, window, cx);
        }
    }

    fn find(&mut self, cx: &mut Context<Self>) {
        let query = self.lookup.read(cx).value().trim().to_lowercase();
        let query = query.trim_start_matches('@');
        self.found = DIRECTORY.iter().position(|p| {
            p.handle.trim_start_matches('@').eq_ignore_ascii_case(query)
                || p.name.to_lowercase().contains(query)
        });
        self.searched = true;
        self.error = None;
        cx.notify();
    }

    fn validate(&mut self, cx: &mut Context<Self>) -> bool {
        if self.found.is_none() {
            self.error = Some("Find a recipient first".into());
            cx.notify();
            return false;
        }
        self.error = None;
        true
    }

    fn card(&self, title: &str, right: Option<AnyElement>, body: impl IntoElement) -> AnyElement {
        let mut card =
            crate::components::section_card::SectionCard::new(title.to_string(), body);
        if let Some(r) = right {
            card = card.trailing(r);
        }
        card.into_any_element()
    }
}

impl Render for SingleChat {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        let fv = view.clone();
        let find_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .w_full()
            .h(px(40.0))
            .px_3()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L2))
            .child(
                svg()
                    .path("icons/magnifying-glass.svg")
                    .w(px(14.0))
                    .h(px(14.0))
                    .flex_shrink_0()
                    .text_color(theme::rgb(theme::colors::MUTED)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.0))
                    .text_color(theme::rgb(theme::colors::INK))
                    .child(gpui_kit::base::input::Input::new(&self.lookup)),
            )
            .child(
                BaseButton::new(ElementId::Name("single-find".into()))
                    .h(px(28.0))
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::radius::DEFAULT))
                    .bg(theme::rgb(theme::colors::PRIMARY))
                    .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                    .text_size(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .on_click(move |_, _, cx: &mut App| {
                        fv.update(cx, |s, cx| s.find(cx));
                    })
                    .child("Find"),
            );

        let result: AnyElement = match (self.searched, self.found) {
            (_, Some(i)) => {
                let peer = &DIRECTORY[i % DIRECTORY.len()];
                let ov = view.clone();
                let sv = view.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w_full()
                    .mt_3()
                    .p_3()
                    .rounded(px(theme::radius::DEFAULT))
                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                    .border_1()
                    .border_color(theme::hsla(theme::colors::PRIMARY, 0.4))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_3()
                            .w_full()
                            .child(
                                div()
                                    .relative()
                                    .w(px(56.0))
                                    .h(px(56.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(theme::radius::DEFAULT))
                                    .bg(theme::rgb(theme::colors::SURFACE_HIGH))
                                    .text_size(px(18.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::rgb(theme::colors::PRIMARY_DARK))
                                    .child(
                                        peer.name
                                            .chars()
                                            .take(1)
                                            .collect::<String>(),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom(px(2.0))
                                            .right(px(2.0))
                                            .w(px(10.0))
                                            .h(px(10.0))
                                            .rounded_full()
                                            .bg(theme::rgb(peer.presence))
                                            .border_1()
                                            .border_color(theme::rgb(0xffffff)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::rgb(
                                                        theme::colors::INK,
                                                    ))
                                                    .child(peer.name),
                                            )
                                            .child(
                                                div()
                                                    .px_1()
                                                    .rounded(px(2.0))
                                                    .bg(theme::rgb(
                                                        theme::colors::SURFACE_HIGH,
                                                    ))
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(9.0))
                                                    .text_color(theme::rgb(
                                                        theme::colors::MUTED,
                                                    ))
                                                    .child(peer.pronouns),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme::rgb(
                                                        theme::colors::PRIMARY,
                                                    ))
                                                    .child(peer.handle),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme::rgb(if peer.online {
                                                        theme::colors::SUCCESS
                                                    } else {
                                                        theme::colors::MUTED
                                                    }))
                                                    .child(if peer.online {
                                                        "• Online now"
                                                    } else {
                                                        "• Offline"
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme::rgb(theme::colors::INK))
                                            .child(peer.bio),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .border_t_1()
                            .border_color(theme::rgb(theme::colors::GRID_L1)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                svg()
                                    .path("icons/users.svg")
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .text_color(theme::rgb(theme::colors::MUTED)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child(peer.mutuals),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .w_full()
                            .child(
                                BaseButton::new(ElementId::Name("single-profile".into()))
                                    .h(px(32.0))
                                    .px_3()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .gap_1()
                                    .rounded(px(theme::radius::DEFAULT))
                                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                                    .border_1()
                                    .border_color(theme::rgb(theme::colors::GRID_L2))
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .on_click(|_, _, cx: &mut App| {
                                        ProfilePage::open(cx);
                                    })
                                    .child(
                                        svg()
                                            .path("icons/users.svg")
                                            .w(px(12.0))
                                            .h(px(12.0))
                                            .text_color(theme::rgb(theme::colors::MUTED)),
                                    )
                                    .child("View Profile"),
                            )
                            .child(
                                BaseButton::new(ElementId::Name("single-open".into()))
                                    .h(px(32.0))
                                    .px_3()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .gap_1()
                                    .rounded(px(theme::radius::DEFAULT))
                                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                                    .border_1()
                                    .border_color(theme::rgb(theme::colors::GRID_L2))
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .on_click(move |_, window, cx: &mut App| {
                                        ov.update(cx, |s, cx| {
                                            s.go(SingleTarget::Chat, window, cx)
                                        });
                                    })
                                    .child(
                                        svg()
                                            .path("icons/chat-bubble-left.svg")
                                            .w(px(12.0))
                                            .h(px(12.0))
                                            .text_color(theme::rgb(theme::colors::MUTED)),
                                    )
                                    .child("Open Chat"),
                            )
                            .child(
                                BaseButton::new(ElementId::Name("single-send".into()))
                                    .h(px(32.0))
                                    .px_3()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .gap_1()
                                    .rounded(px(theme::radius::DEFAULT))
                                    .bg(theme::rgb(theme::colors::PRIMARY))
                                    .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::BOLD)
                                    .on_click({
                                        let sv2 = sv.clone();
                                        move |_, window, cx: &mut App| {
                                            if sv.update(cx, |s, cx| s.validate(cx)) {
                                                sv2.update(cx, |s, cx| {
                                                    s.go(SingleTarget::Chat, window, cx)
                                                });
                                            }
                                        }
                                    })
                                    .child(
                                        svg()
                                            .path("icons/paper-airplane.svg")
                                            .w(px(12.0))
                                            .h(px(12.0))
                                            .text_color(theme::rgb(
                                                theme::colors::ON_PRIMARY,
                                            )),
                                    )
                                    .child("Send Message"),
                            ),
                    )
                    .into_any_element()
            }
            (true, None) => div()
                .mt_3()
                .text_size(px(12.0))
                .text_color(theme::rgb(theme::colors::MUTED))
                .child("No peer found. Try @dariakova, @jthorne, or @anyalind.")
                .into_any_element(),
            (false, _) => div()
                .mt_3()
                .text_size(px(12.0))
                .text_color(theme::rgb(theme::colors::MUTED))
                .child("Type a handle and press Find.")
                .into_any_element(),
        };

        let gv = view.clone();
        div()
            .flex()
            .flex_col()
            .items_center()
            .size_full()
            .overflow_hidden()
            .bg(theme::rgb(theme::colors::BACKGROUND))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .flex_grow(1.0)
                    .min_w_0()
                    .id(ElementId::Name("single-scroll".into()))
                    .overflow_y_scroll()
                    .p_5()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                svg()
                                                    .path("icons/users.svg")
                                                    .w(px(12.0))
                                                    .h(px(12.0))
                                                    .text_color(theme::rgb(
                                                        theme::colors::PRIMARY,
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(10.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::rgb(
                                                        theme::colors::PRIMARY,
                                                    ))
                                                    .child("NEW CONVERSATION / SINGLE CHAT"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .font_family(theme::fonts::HEADLINE)
                                            .text_size(px(22.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::rgb(theme::colors::INK))
                                            .child("Start Single Chat"),
                                    ),
                            )
                            .child(
                                MonoLink::new("New group instead").on_click(
                                    move |_, window, cx: &mut App| {
                                        gv.update(cx, |s, cx| {
                                            s.go(SingleTarget::Group, window, cx)
                                        });
                                    },
                                ),
                            ),
                    )
                    .child({
                        let notice: AnyElement = if let Some(error) = self.error.clone() {
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::rgb(theme::colors::ERROR))
                                .child(error)
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        };
                        notice
                    })
                    .child(
                        div().w_full().child(self.card(
                            "RECIPIENT LOOKUP",
                            Some(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .text_color(theme::rgb(
                                        theme::colors::MUTED,
                                    ))
                                    .child("Direct Peer Protocol")
                                    .into_any_element(),
                            ),
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .w_full()
                                .child(find_row)
                                .child(result),
                        )),
                    ),
            )
    }
}

/// Live conversation-security card for the inspector sheet. Holds the
/// single-chat form entity so toggles stay in sync with the canvas.
pub struct SingleSecuritySheet {
    form: Entity<SingleChat>,
    _watches: Vec<Subscription>,
}

impl SingleSecuritySheet {
    pub fn new(form: Entity<SingleChat>, cx: &mut Context<Self>) -> Self {
        let watches = vec![cx.observe(&form, |_, _, cx| cx.notify())];
        Self {
            form,
            _watches: watches,
        }
    }
}

impl Render for SingleSecuritySheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let (disappearing, receipts, autodl) =
            self.form.read_with(cx, |form, _| {
                (form.disappearing, form.receipts, form.autodl)
            });
        let timer = TIMERS[disappearing % TIMERS.len()];

        let tv = view.clone();
        let rv = view.clone();
        let av = view.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .p_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::INK))
                            .child("CONVERSATION SECURITY"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .px_1()
                                    .rounded(px(2.0))
                                    .bg(theme::hsla(theme::colors::SUCCESS, 0.14))
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::rgb(theme::colors::SUCCESS))
                                    .child("Standard E2EE"),
                            )
                            .child(
                                BaseButton::new(ElementId::Name("security-close".into()))
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(2.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .hover(|s| s.bg(theme::rgb(theme::colors::ROW_HOVER)))
                                    .on_click(|_, window, cx| window.close_sheet(cx))
                                    .child(
                                        svg()
                                            .path("icons/x-mark.svg")
                                            .w(px(12.0))
                                            .h(px(12.0))
                                            .text_color(theme::rgb(theme::colors::MUTED)),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .w_full()
                    .p_3()
                    .rounded(px(theme::radius::DEFAULT))
                    .bg(theme::hsla(theme::colors::PRIMARY, 0.05))
                    .border_1()
                    .border_color(theme::hsla(theme::colors::PRIMARY, 0.4))
                    .child(
                        div()
                            .w(px(36.0))
                            .h(px(36.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::radius::DEFAULT))
                            .bg(theme::rgb(theme::colors::PRIMARY))
                            .child(
                                svg()
                                    .path("icons/lock-closed.svg")
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child("End-to-End Encryption"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("Messages and files are encrypted directly between devices."),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child("Disappearing Messages"),
                            )
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("Self-destruct timer"),
                            ),
                    )
                    .child(
                        BaseButton::new(ElementId::Name("single-timer".into()))
                            .w_full()
                            .h(px(36.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .px_3()
                            .rounded(px(theme::radius::DEFAULT))
                            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                            .border_1()
                            .border_color(theme::rgb(theme::colors::GRID_L2))
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::INK))
                            .on_click(move |_, _, cx: &mut App| {
                                tv.update(cx, |s, cx| {
                                    s.form.update(cx, |f, cx| {
                                        f.disappearing =
                                            (f.disappearing + 1) % TIMERS.len();
                                        cx.notify();
                                    });
                                });
                            })
                            .child(timer)
                            .child(
                                div().ml_auto().child(
                                    svg()
                                        .path("icons/chevron-down.svg")
                                        .w(px(12.0))
                                        .h(px(12.0))
                                        .text_color(theme::rgb(theme::colors::MUTED)),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child("Read Receipts"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("Notify recipient when messages are read"),
                            ),
                    )
                    .child(
                        Switch::new(ElementId::Name("single-receipts".into()))
                            .checked(receipts)
                            .color(theme::rgb(theme::colors::PRIMARY))
                            .on_change(move |checked: &bool, _, cx: &mut App| {
                                let on = *checked;
                                rv.update(cx, |s, cx| {
                                    s.form.update(cx, |f, cx| {
                                        f.receipts = on;
                                        cx.notify();
                                    });
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child("Media Auto-Download"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("Cache incoming files and high-res schematics"),
                            ),
                    )
                    .child(
                        Switch::new(ElementId::Name("single-autodl".into()))
                            .checked(autodl)
                            .color(theme::rgb(theme::colors::PRIMARY))
                            .on_change(move |checked: &bool, _, cx: &mut App| {
                                let on = *checked;
                                av.update(cx, |s, cx| {
                                    s.form.update(cx, |f, cx| {
                                        f.autodl = on;
                                        cx.notify();
                                    });
                                });
                            }),
                    ),
            )
    }
}
