//! Profile / preferences page — ChatWindow shell, profile content.
//!
//! Same application shell as the chat window (`TopBar` + `IconRail` +
//! resizable panels): left preferences nav with working filter and
//! section switching, center editable profile cards, right live preview
//! card mirroring every keystroke. Bio stays single-line (the shared
//! input state here is single-line mode).

use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::input::InputState as BaseInput;
use gpui_kit::base::{ResizableState, h_resizable, resizable_panel};
use gpui_kit::component::input::InputState as KitInput;
use gpui_kit::component::{Placement, WindowExt};
use gpui_kit::*;

use crate::components::form_field::FormField;
use crate::components::side_search_bar::SideSearchBar;
use crate::layout::icon_rail::IconRail;
use crate::layout::top_bar::TopBar;
use crate::theme;

/// Locked content size of the profile window: chat-window parity.
/// Single source for the `main.rs` bounds.
pub const PROFILE_WINDOW_SIZE: [f32; 2] = [1000.0, 700.0];

struct NavItem {
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    tag: Option<&'static str>,
}

struct NavGroup {
    title: &'static str,
    items: &'static [NavItem],
}

const NAV: &[NavGroup] = &[
    NavGroup {
        title: "IDENTITY & PROFILE",
        items: &[
            NavItem {
                id: "my-profile",
                label: "My Profile",
                icon: "icons/users.svg",
                tag: Some("ACTIVE"),
            },
            NavItem {
                id: "account",
                label: "Account & Credentials",
                icon: "icons/lock-closed.svg",
                tag: None,
            },
            NavItem {
                id: "clearance",
                label: "Access Clearance & Keys",
                icon: "icons/shield-check.svg",
                tag: None,
            },
        ],
    },
    NavGroup {
        title: "WORKSPACE CLIENT",
        items: &[
            NavItem {
                id: "themes",
                label: "Themes & Display",
                icon: "icons/sun.svg",
                tag: Some("CAD BLUE"),
            },
            NavItem {
                id: "notifications",
                label: "Notifications & Badges",
                icon: "icons/moon.svg",
                tag: Some("3"),
            },
            NavItem {
                id: "chat-fmt",
                label: "Chat & Formatting",
                icon: "icons/chat-bubble-left.svg",
                tag: None,
            },
            NavItem {
                id: "audio",
                label: "Audio & Hardware",
                icon: "icons/cog-6-tooth.svg",
                tag: None,
            },
            NavItem {
                id: "devices",
                label: "Devices & Sessions",
                icon: "icons/squares-2x2.svg",
                tag: None,
            },
        ],
    },
];

/// Presence options: label + dot color.
const PRESENCE: &[(&str, u32)] = &[
    ("Online & Available", theme::colors::SUCCESS),
    ("Away", theme::colors::WARNING),
    ("Busy", theme::colors::CRITICAL),
    ("Offline", theme::colors::MUTED),
];

/// Avatar presets: initials + bg + fg. `None` = default account avatar.
const AVATARS: &[(&str, u32, u32)] = &[
    ("ER", 0xdbeafe, 0x1d4ed8),
    ("ER", 0xe0e7ff, 0x3730a3),
    ("ER", 0xd1fae5, 0x065f46),
];

/// Banner hues cycled by Edit Banner.
const BANNERS: &[u32] = &[
    theme::colors::PRIMARY,
    theme::colors::PRIMARY_DARK,
    theme::colors::TERTIARY_CONTAINER,
];

pub struct ProfilePage {
    theme: u8,
    layout: Entity<ResizableState>,
    nav_search: Entity<KitInput>,
    section: &'static str,
    name: Entity<BaseInput>,
    username: Entity<BaseInput>,
    bio: Entity<BaseInput>,
    status: Entity<BaseInput>,
    presence: usize,
    avatar: Option<usize>,
    banner: usize,
    banner_pick: bool,
    _watches: Vec<Subscription>,
}

impl ProfilePage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let nav_search = cx.new(|cx| {
            KitInput::new(window, cx).placeholder("Filter preferences...")
        });
        let name = cx.new(|cx| BaseInput::new(window, cx).default_value("Elena Rostova"));
        let username = cx.new(|cx| BaseInput::new(window, cx).default_value("@rostova_cad"));
        let bio = cx.new(|cx| {
            BaseInput::new(window, cx).default_value(
                "Building real-time WebGL/WGSL visual processing pipelines. Ping me for shader reviews.",
            )
        });
        let status = cx.new(|cx| {
            BaseInput::new(window, cx)
                .default_value("Working on shader pipeline #image-processing")
        });
        let fields = [&name, &username, &bio, &status];
        let mut _watches: Vec<Subscription> = fields
            .iter()
            .map(|field| cx.observe(*field, |_, _, cx| cx.notify()))
            .collect();
        _watches.push(cx.observe(&nav_search, |_, _, cx| cx.notify()));
        Self {
            theme: 1,
            layout: cx.new(|_| ResizableState::default()),
            nav_search,
            section: "my-profile",
            name,
            username,
            bio,
            status,
            presence: 0,
            avatar: Some(0),
            banner: 0,
            banner_pick: false,
            _watches,
        }
    }

    pub fn open(cx: &mut App) -> bool {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(
                    px(PROFILE_WINDOW_SIZE[0]),
                    px(PROFILE_WINDOW_SIZE[1]),
                ),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Profile".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        gpui_kit::open_window(opts, cx, |window, cx| {
            cx.new(|cx| ProfilePage::new(window, cx))
        })
        .is_ok()
    }

    fn open_preview(&self, window: &mut Window, cx: &mut App) {
        let preview = cx.new(|cx| PreviewSheet::new(self, cx));
        window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
            sheet
                .title("Profile Preview")
                .size(px(340.0))
                .resizable(true)
                .overlay(false)
                .overlay_closable(false)
                .child(preview.clone())
        });
    }

    fn set_theme(&mut self, index: u8, cx: &mut Context<Self>) {
        self.theme = index.min(2);
        cx.notify();
    }

    fn set_section(&mut self, id: &'static str, cx: &mut Context<Self>) {
        self.section = id;
        cx.notify();
    }

    fn cycle_avatar(&mut self, cx: &mut Context<Self>) {
        self.avatar = match self.avatar {
            None => Some(0),
            Some(i) if i + 1 < AVATARS.len() => Some(i + 1),
            _ => None,
        };
        cx.notify();
    }

    fn toggle_banner_pick(&mut self, cx: &mut Context<Self>) {
        self.banner_pick = !self.banner_pick;
        cx.notify();
    }

    fn set_banner(&mut self, index: usize, cx: &mut Context<Self>) {
        self.banner = index % BANNERS.len();
        self.banner_pick = false;
        cx.notify();
    }

    fn clear_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.status.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        cx.notify();
    }

    fn render_nav(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let query = self.nav_search.read(cx).value().to_lowercase();
        let active = self.section;
        let mut col = div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .child(SideSearchBar::new(self.nav_search.clone()).shortcut(""));
        for group in NAV {
            let items: Vec<&NavItem> = group
                .items
                .iter()
                .filter(|item| {
                    query.is_empty() || item.label.to_lowercase().contains(&query)
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            let mut list = div().flex().flex_col().gap_1().px_3();
            for item in items {
                let is_active = active == item.id;
                let mut row = BaseButton::new(ElementId::Name(
                    format!("prefs-{}", item.id).into(),
                ))
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .justify_start()
                .gap_2()
                .px_3()
                .h(px(32.0))
                .rounded(px(theme::radius::DEFAULT))
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM);
                row = if is_active {
                    row.bg(theme::rgb(theme::colors::PRIMARY))
                        .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                } else {
                    row.text_color(theme::rgb(theme::colors::INK))
                        .hover(|s| s.bg(theme::rgb(theme::colors::ROW_HOVER)))
                };
                let mut row = row
                    .child(
                        svg()
                            .path(item.icon)
                            .w(px(14.0))
                            .h(px(14.0))
                            .flex_shrink_0()
                            .text_color(if is_active {
                                theme::rgb(theme::colors::ON_PRIMARY)
                            } else {
                                theme::rgb(theme::colors::MUTED)
                            }),
                    )
                    .child(item.label);
                if let Some(tag) = item.tag {
                    let chip = if is_active {
                        div()
                            .ml_auto()
                            .px_1()
                            .rounded(px(2.0))
                            .bg(theme::hsla(0xffffff, 0.25))
                            .font_family(theme::fonts::MONO)
                            .text_size(px(9.0))
                            .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                            .child(tag)
                    } else if item.id == "notifications" {
                        div()
                            .ml_auto()
                            .px_1()
                            .rounded(px(2.0))
                            .bg(theme::hsla(theme::colors::PRIMARY, 0.12))
                            .font_family(theme::fonts::MONO)
                            .text_size(px(9.0))
                            .text_color(theme::rgb(theme::colors::PRIMARY))
                            .child(tag)
                    } else {
                        div()
                            .ml_auto()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(9.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child(tag)
                    };
                    row = row.child(chip);
                }
                let v = view.clone();
                let sid = item.id;
                list = list.child(row.on_click(move |_, _, cx: &mut App| {
                    v.update(cx, |p, cx| p.set_section(sid, cx));
                }));
            }
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .py_2()
                    .child(
                        div()
                            .px_3()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child(group.title),
                    )
                    .child(list),
            );
        }
        col
    }

    /// Circular initial-avatar. Local on purpose: the shared `Avatar`
    /// is square by design; the profile header wants a ringed circle.
    fn render_face(&self, size: f32) -> impl IntoElement {
        let (initial, bg, fg) = match self.avatar {
            Some(i) => AVATARS[i % AVATARS.len()],
            None => ("ER", theme::colors::INSET, theme::colors::MUTED),
        };
        div()
            .w(px(size))
            .h(px(size))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(theme::rgb(bg))
            .border_2()
            .border_color(theme::rgb(0xffffff))
            .text_size(px(size * 0.36))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::rgb(fg))
            .child(initial)
    }

    fn render_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let name = self.name.read(cx).value().to_string();
        let handle = self.username.read(cx).value().to_string();
        let bio_len = self.bio.read(cx).value().chars().count();

        let sv = view.clone();
        let status_box = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .h(px(36.0))
            .px_3()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L2))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.0))
                    .text_color(theme::rgb(theme::colors::INK))
                    .child(gpui_kit::base::input::Input::new(&self.status)),
            )
            .child(
                BaseButton::new(ElementId::Name("profile-status-clear".into()))
                    .w(px(20.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(2.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .on_click(move |_, window, cx: &mut App| {
                        sv.update(cx, |p, cx| p.clear_status(window, cx));
                    })
                    .child(
                        svg()
                            .path("icons/x-mark.svg")
                            .w(px(12.0))
                            .h(px(12.0))
                            .text_color(theme::rgb(theme::colors::MUTED)),
                    ),
            );

        div()
            .flex()
            .flex_col()
            .items_center()
            .w_full()
            .p_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .flex_grow(1.0)
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .rounded(px(theme::radius::DEFAULT))
                            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                            .border_1()
                            .border_color(theme::rgb(theme::colors::GRID_L1))
                            .overflow_hidden()
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .aspect_ratio(6.0)
                                    .bg(theme::rgb(BANNERS[self.banner % BANNERS.len()]))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .w_full()
                                            .h_full()
                                            .px_4()
                                            .child(
                                                div()
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(10.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme::rgb(
                                                        theme::colors::ON_PRIMARY,
                                                    ))
                                                    .child(
                                                        "SCHEMATIC_MATRIX // GRAPHICS_LAB_V4",
                                                    ),
                                            ),
                                    )
                                    .child({
                                        let bv = view.clone();
                                        BaseButton::new(ElementId::Name(
                                            "profile-banner-edit".into(),
                                        ))
                                        .absolute()
                                        .right(px(12.0))
                                        .bottom(px(-12.0))
                                        .w(px(24.0))
                                        .h(px(24.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                                        .border_1()
                                        .border_color(theme::rgb(theme::colors::GRID_L2))
                                        .text_color(theme::rgb(theme::colors::MUTED))
                                        .on_click(move |_, _, cx: &mut App| {
                                            bv.update(cx, |p, cx| p.toggle_banner_pick(cx));
                                        })
                                        .child(
                                            svg()
                                                .path("icons/pencil.svg")
                                                .w(px(12.0))
                                                .h(px(12.0))
                                                .text_color(theme::rgb(
                                                    theme::colors::MUTED,
                                                )),
                                        )
                                    }),
                            )
                            .child(if self.banner_pick {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .w_full()
                                    .px_4()
                                    .py_2()
                                    .bg(theme::rgb(theme::colors::SURFACE_LOW))
                                    .border_b_1()
                                    .border_color(theme::rgb(theme::colors::GRID_L1))
                                    .children((0..BANNERS.len()).map(|i| {
                                        let sv = view.clone();
                                        let mut sw = BaseButton::new(ElementId::Name(
                                            format!("profile-swatch-{i}").into(),
                                        ))
                                        .w(px(28.0))
                                        .h(px(20.0))
                                        .rounded(px(2.0))
                                        .bg(theme::rgb(BANNERS[i]))
                                        .border_1();
                                        sw = if i == self.banner {
                                            sw.border_color(theme::rgb(theme::colors::INK))
                                        } else {
                                            sw.border_color(theme::rgb(
                                                theme::colors::GRID_L2,
                                            ))
                                        };
                                        sw.on_click(move |_, _, cx: &mut App| {
                                            sv.update(cx, |p, cx| p.set_banner(i, cx));
                                        })
                                    }))
                            } else {
                                div()
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .px_4()
                                    .py_3()
                                    .child(
                                        div()
                                            .relative()
                                            .mt(px(-28.0))
                                            .child(self.render_face(64.0))
                                            .child({
                                                let av = view.clone();
                                                BaseButton::new(ElementId::Name(
                                                    "profile-avatar-edit".into(),
                                                ))
                                                .absolute()
                                                .right(px(-4.0))
                                                .bottom(px(-4.0))
                                                .w(px(24.0))
                                                .h(px(24.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .rounded_full()
                                                .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                                                .border_1()
                                                .border_color(theme::rgb(theme::colors::GRID_L2))
                                                .text_color(theme::rgb(theme::colors::MUTED))
                                                .on_click(move |_, _, cx: &mut App| {
                                                    av.update(cx, |p, cx| p.cycle_avatar(cx));
                                                })
                                                .child(
                                                    svg()
                                                        .path("icons/pencil.svg")
                                                        .w(px(12.0))
                                                        .h(px(12.0))
                                                        .text_color(theme::rgb(
                                                            theme::colors::MUTED,
                                                        )),
                                                )
                                            }),
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
                                                            .text_size(px(16.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(theme::rgb(
                                                                theme::colors::ON_SURFACE,
                                                            ))
                                                            .child(SharedString::from(name.clone())),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_1()
                                                            .rounded(px(2.0))
                                                            .bg(theme::hsla(
                                                                theme::colors::PRIMARY,
                                                                0.12,
                                                            ))
                                                            .font_family(theme::fonts::MONO)
                                                            .text_size(px(9.0))
                                                            .text_color(theme::rgb(
                                                                theme::colors::PRIMARY,
                                                            ))
                                                            .child("L4 Architect"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme::rgb(
                                                        theme::colors::MUTED,
                                                    ))
                                                    .child(SharedString::from(handle)),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .w_full()
                            .mt_4()
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("CUSTOM_STATUS_STRING"),
                            )
                            .child(status_box),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .mt_4()
                            .rounded(px(theme::radius::DEFAULT))
                            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                            .border_1()
                            .border_color(theme::rgb(theme::colors::GRID_L1))
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
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                svg()
                                                    .path("icons/users.svg")
                                                    .w(px(14.0))
                                                    .h(px(14.0))
                                                    .flex_shrink_0()
                                                    .text_color(theme::rgb(
                                                        theme::colors::PRIMARY,
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::rgb(
                                                        theme::colors::ON_SURFACE,
                                                    ))
                                                    .child(
                                                        "Identity & Specification Parameters",
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .font_family(theme::fonts::MONO)
                                            .text_size(px(9.0))
                                            .text_color(theme::rgb(theme::colors::MUTED))
                                            .child("LOC: S1-01 // PARAMS"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .w_full()
                                    .mt_3()
                                    .child(
                                        div().flex_1().min_w_0().child(
                                            FormField::new("FULL NAME *", self.name.clone()),
                                        ),
                                    )
                                    .child(
                                        div().flex_1().min_w_0().child(
                                            FormField::new(
                                                "USERNAME / CAD HANDLE *",
                                                self.username.clone(),
                                            )
                                            .header_extra(
                                                div()
                                                    .px_1()
                                                    .rounded(px(2.0))
                                                    .bg(theme::hsla(
                                                        theme::colors::SUCCESS,
                                                        0.14,
                                                    ))
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(9.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme::rgb(
                                                        theme::colors::SUCCESS,
                                                    ))
                                                    .child("VERIFIED"),
                                            ),
                                        ),
                                    ),
                            )
                            .child(
                                div().w_full().mt_3().child(
                                    FormField::new("BIO / TECHNICAL STATEMENT", self.bio.clone())
                                        .header_extra(
                                            div()
                                                .font_family(theme::fonts::MONO)
                                                .text_size(px(9.0))
                                                .text_color(theme::rgb(theme::colors::MUTED))
                                                .child(SharedString::from(format!(
                                                    "{} / 250",
                                                    bio_len.min(250)
                                                ))),
                                        ),
                                ),
                            ),
                    ),
            )
    }

    fn render_stub(&self, title: &'static str) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .w_full()
            .p_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w_full()
                    .rounded(px(theme::radius::DEFAULT))
                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .p_5()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::rgb(theme::colors::ON_SURFACE))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("This section follows the same blueprint. Content lands here next."),
                    ),
            )
    }

    fn meta_row(label: &'static str, value: AnyElement) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .child(
                div()
                    .font_family(theme::fonts::MONO)
                    .text_size(px(9.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child(label),
            )
            .child(value)
            .into_any_element()
    }
}

/// Live profile preview card for the inspector sheet. Holds clones of
/// the page's text entities (re-rendered on every keystroke) plus the
/// presence/avatar/banner values snapshotted when the sheet opens.
pub struct PreviewSheet {
    name: Entity<BaseInput>,
    username: Entity<BaseInput>,
    bio: Entity<BaseInput>,
    status: Entity<BaseInput>,
    presence: usize,
    avatar: Option<usize>,
    banner: usize,
    _watches: Vec<Subscription>,
}

impl PreviewSheet {
    fn new(page: &ProfilePage, cx: &mut Context<Self>) -> Self {
        let entities = [
            &page.name,
            &page.username,
            &page.bio,
            &page.status,
        ];
        let _watches: Vec<Subscription> = entities
            .iter()
            .map(|entity| cx.observe(*entity, |_, _, cx| cx.notify()))
            .collect();
        Self {
            name: page.name.clone(),
            username: page.username.clone(),
            bio: page.bio.clone(),
            status: page.status.clone(),
            presence: page.presence,
            avatar: page.avatar,
            banner: page.banner,
            _watches,
        }
    }

    /// Circular initial-avatar, matching the page header.
    fn face(&self, size: f32) -> impl IntoElement {
        let (initial, bg, fg) = match self.avatar {
            Some(i) => AVATARS[i % AVATARS.len()],
            None => ("ER", theme::colors::INSET, theme::colors::MUTED),
        };
        div()
            .w(px(size))
            .h(px(size))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(theme::rgb(bg))
            .border_2()
            .border_color(theme::rgb(0xffffff))
            .text_size(px(size * 0.36))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::rgb(fg))
            .child(initial)
    }
}

impl Render for PreviewSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let presence_label = PRESENCE[self.presence % PRESENCE.len()].0;
        let name = self.name.read(cx).value().to_string();
        let handle = self.username.read(cx).value().to_string();
        let bio = self.bio.read(cx).value().to_string();
        let status = self.status.read(cx).value().to_string();
        let value = |text: String| {
            div()
                .text_size(px(10.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::rgb(theme::colors::INK))
                .child(SharedString::from(text))
                .into_any_element()
        };
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
                            .child("PROFILE PREVIEW // CARD"),
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
                                    .bg(theme::hsla(theme::colors::PRIMARY, 0.12))
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .text_color(theme::rgb(theme::colors::PRIMARY))
                                    .child("LIVE CAD"),
                            )
                            .child(
                                BaseButton::new(ElementId::Name("preview-close".into()))
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(2.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .hover(|s| {
                                        s.bg(theme::rgb(theme::colors::ROW_HOVER))
                                    })
                                    .on_click(|_, window, cx| window.close_sheet(cx))
                                    .child(
                                        svg()
                                            .path("icons/x-mark.svg")
                                            .w(px(12.0))
                                            .h(px(12.0))
                                            .text_color(theme::rgb(
                                                theme::colors::MUTED,
                                            )),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child("This is how your operator persona appears to team members in channels and feeds."),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .rounded(px(theme::radius::DEFAULT))
                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .bg(theme::rgb(BANNERS[self.banner % BANNERS.len()]))
                            .aspect_ratio(6.0)
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(8.0))
                                    .text_color(theme::hsla(0xffffff, 0.75))
                                    .child("DWG : 804"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .px_3()
                            .pb_3()
                            .child(div().mt(px(-20.0)).child(self.face(40.0)))
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
                                            .flex_row()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::rgb(
                                                        theme::colors::ON_SURFACE,
                                                    ))
                                                    .child(SharedString::from(name)),
                                            )
                                            .child(
                                                div()
                                                    .px_1()
                                                    .rounded(px(2.0))
                                                    .bg(theme::hsla(
                                                        theme::colors::PRIMARY,
                                                        0.12,
                                                    ))
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(8.0))
                                                    .text_color(theme::rgb(
                                                        theme::colors::PRIMARY,
                                                    ))
                                                    .child("L4"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_1()
                                            .rounded(px(2.0))
                                            .border_1()
                                            .border_color(theme::rgb(theme::colors::SUCCESS))
                                            .font_family(theme::fonts::MONO)
                                            .text_size(px(9.0))
                                            .text_color(theme::rgb(theme::colors::SUCCESS))
                                            .child(SharedString::from(format!(
                                                "● {}",
                                                presence_label
                                            ))),
                                    ),
                            )
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(10.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child(SharedString::from(handle)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child(SharedString::from(if status.is_empty() {
                                        "No custom status set.".to_string()
                                    } else {
                                        status
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child(SharedString::from(bio)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .w_full()
                                    .mt_2()
                                    .child(ProfilePage::meta_row("TIMEZONE", value("UTC-8 (PST)".into())))
                                    .child(ProfilePage::meta_row(
                                        "CLUSTER",
                                        value("Core Graphics".into()),
                                    ))
                                    .child(ProfilePage::meta_row(
                                        "CLEARANCE",
                                        div()
                                            .font_family(theme::fonts::MONO)
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::rgb(theme::colors::PRIMARY))
                                            .child("L4 Architect")
                                            .into_any_element(),
                                    )),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .w_full()
                    .p_3()
                    .rounded(px(theme::radius::DEFAULT))
                    .bg(theme::hsla(theme::colors::PRIMARY, 0.05))
                    .border_1()
                    .border_color(theme::hsla(theme::colors::PRIMARY, 0.35))
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
                                    .text_size(px(9.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("PEER DISCOVERY"),
                            )
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::rgb(theme::colors::SUCCESS))
                                    .child("ACTIVE"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("Card broadcasts to 14 channels in the clusters you follow."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .mt_auto()
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(9.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("NODE: 0x02A"),
                    )
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(9.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("SYS_STATUS: NOMINAL"),
                    ),
            )
    }
}

impl Render for ProfilePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let view = cx.entity().clone();
        let pv = cx.entity().clone();
        let sv = cx.entity().clone();
        let center: AnyElement = if self.section == "my-profile" {
            self.render_form(cx).into_any_element()
        } else {
            let title = NAV
                .iter()
                .flat_map(|g| g.items.iter())
                .find(|i| i.id == self.section)
                .map(|i| i.label)
                .unwrap_or("Section");
            self.render_stub(title).into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::rgb(theme::colors::BACKGROUND))
            .child(
                TopBar::new("Varta — Profile").on_inspector(move |_, window, cx| {
                    pv.update(cx, |p, cx| p.open_preview(window, cx));
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
                            .on_settings(move |_, window, cx| {
                                sv.update(cx, |p, cx| p.open_preview(window, cx));
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                h_resizable("varta-profile")
                                    .with_state(&self.layout)
                                    .child(
                                        resizable_panel()
                                            .size(px(260.0))
                                            .size_range(px(220.0)..Pixels::MAX)
                                            .child(self.render_nav(cx)),
                                    )
                                    .child(resizable_panel().child(center)),
                            ),
                    ),
            )
    }
}
