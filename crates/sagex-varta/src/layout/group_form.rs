//! New-group form — layout for the ChatWindow canvas slot.
//!
//! Same pattern as the other canvas content: parent-owned entities, all
//! controls live. The members column lives in the inspector sheet
//! (`GroupMembersSheet` holds the form entity); the canvas shows header,
//! profile/aesthetics, privacy, and permissions.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::actions::Cancel;
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::WindowExt;
use gpui_kit::*;

use crate::components::form_field::FormField;
use crate::components::mono_link::MonoLink;
use crate::theme;

type BackHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Group accent swatches: label + hex.
const ACCENTS: &[(&str, u32)] = &[
    ("Cobalt", 0x2563eb),
    ("Pink", 0xec4899),
    ("Emerald", 0x059669),
    ("Amber", 0xd97706),
    ("Violet", 0x7c3aed),
    ("Cyan", 0x0284c7),
];

struct PrivacyOption {
    title: &'static str,
    icon: &'static str,
    desc: &'static str,
    access: &'static str,
}

const PRIVACY: &[PrivacyOption] = &[
    PrivacyOption {
        title: "Public Circle",
        icon: "icons/globe.svg",
        desc: "Discoverable on explore matrix. Anyone can preview and join via link.",
        access: "Access: Unrestricted",
    },
    PrivacyOption {
        title: "Private Friends",
        icon: "icons/lock-closed.svg",
        desc: "Invite-only. Host approval required. Trusted social circle for close hangouts.",
        access: "Access: Mutual Friends Only",
    },
    PrivacyOption {
        title: "Ghost Lounge",
        icon: "icons/moon.svg",
        desc: "Hidden from direct search. Stealth token link access with burn on exit.",
        access: "Access: Stealth Key Only",
    },
];

struct PermDef {
    title: &'static str,
    icon: &'static str,
    desc: &'static str,
}

const PERMS: &[PermDef] = &[
    PermDef {
        title: "High-Res Media & Voice Notes",
        icon: "icons/photo.svg",
        desc: "Permit 4K beach photo dumps, audio clips up to 5 min, and raw meme uploads.",
    },
    PermDef {
        title: "Member Invite Links",
        icon: "icons/link.svg",
        desc: "Allow any member in the circle to invite mutual friends and roommates.",
    },
    PermDef {
        title: "Sticker & Emoji Reactions",
        icon: "icons/face-smile.svg",
        desc: "Enable animated sticker packs, custom emoji reactions, and live tap reactions.",
    },
    PermDef {
        title: "Disappearing Messages",
        icon: "icons/timer.svg",
        desc: "Auto-scrub memes and messages after 24 hours (keep history for weekend plans).",
    },
];

struct Suggested {
    name: &'static str,
    handle: &'static str,
    initials: &'static str,
    presence: u32,
}

const SUGGESTED: &[Suggested] = &[
    Suggested {
        name: "Daria Kova",
        handle: "@dariakova",
        initials: "DK",
        presence: theme::colors::SUCCESS,
    },
    Suggested {
        name: "Julian Thorne",
        handle: "@jthorne",
        initials: "JT",
        presence: theme::colors::SUCCESS,
    },
    Suggested {
        name: "Anya Lindqvist",
        handle: "@anyalind",
        initials: "AL",
        presence: theme::colors::MUTED,
    },
];

#[derive(Clone)]
pub(crate) struct Member {
    handle: String,
    initials: String,
}

fn initials_of(handle: &str) -> String {
    let clean: String = handle
        .trim_start_matches('@')
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(2)
        .collect();
    clean.to_uppercase()
}

/// Canonical slug derived from the group name.
fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        "new-group".to_string()
    } else {
        slug
    }
}

pub struct GroupForm {
    name: Entity<InputState>,
    description: Entity<InputState>,
    member_search: Entity<InputState>,
    added: Vec<Member>,
    accent: usize,
    privacy: usize,
    perms: [bool; 4],
    copied: bool,
    error: Option<SharedString>,
    created: bool,
    on_back: Option<BackHandler>,
    _watches: Vec<Subscription>,
}

impl GroupForm {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Weekend Beach Hangout"));
        let description =
            cx.new(|cx| InputState::new(window, cx).placeholder("What is this circle about?"));
        let member_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("@ username (e.g. alex, elena)")
        });
        let fields = [&name, &description, &member_search];
        let mut _watches: Vec<Subscription> = fields
            .iter()
            .map(|field| {
                cx.observe(*field, |view: &mut Self, _, cx| {
                    view.error = None;
                    view.created = false;
                    cx.notify();
                })
            })
            .collect();
        _watches.push(cx.subscribe(
            &member_search,
            |view: &mut Self, _: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    view.add_member(cx);
                }
            },
        ));
        Self {
            name,
            description,
            member_search,
            added: Vec::new(),
            accent: 0,
            privacy: 1,
            perms: [true, true, true, false],
            copied: false,
            error: None,
            created: false,
            on_back: None,
            _watches,
        }
    }

    pub fn on_back(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Rc::new(handler));
        self
    }

    pub(crate) fn slug(&self, cx: &mut Context<Self>) -> String {
        slugify(&self.name.read(cx).value())
    }

    pub(crate) fn members(&self) -> &[Member] {
        &self.added
    }

    fn add_member(&mut self, cx: &mut Context<Self>) {
        let raw = self.member_search.read(cx).value().trim().to_string();
        if raw.is_empty() {
            return;
        }
        let handle = if raw.starts_with('@') {
            raw
        } else {
            format!("@{raw}")
        };
        if !self
            .added
            .iter()
            .any(|m| m.handle.eq_ignore_ascii_case(&handle))
        {
            let initials = initials_of(&handle);
            self.added.push(Member { handle, initials });
        }
        // Note: the search box keeps its text (clearing needs a `Window`,
        // which the Enter-key path does not receive). The new row + counter
        // below confirm the add.
        cx.notify();
    }

    fn add_handle(&mut self, handle: &str, cx: &mut Context<Self>) {
        if !self
            .added
            .iter()
            .any(|m| m.handle.eq_ignore_ascii_case(handle))
        {
            self.added.push(Member {
                handle: handle.to_string(),
                initials: initials_of(handle),
            });
        }
        cx.notify();
    }

    fn remove_member(&mut self, handle: &str, cx: &mut Context<Self>) {
        self.added.retain(|m| m.handle != handle);
        cx.notify();
    }

    fn try_create(&mut self, cx: &mut Context<Self>) {
        if self.name.read(cx).value().trim().is_empty() {
            self.error = Some("Group name is required".into());
            self.created = false;
            cx.notify();
            return;
        }
        self.error = None;
        self.created = true;
        cx.notify();
    }

    fn render_card(&self, title: &str, body: impl IntoElement) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child(
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
                    .child(div().w(px(6.0)).h(px(6.0)).bg(theme::rgb(theme::colors::PRIMARY)))
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::INK))
                            .child(title.to_string()),
                    ),
            )
            .child(div().w_full().p_4().child(body))
            .into_any_element()
    }
}

impl Render for GroupForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let slug = self.slug(cx);
        let desc_len = self.description.read(cx).value().chars().count();
        let theme_accent = ACCENTS[self.accent % ACCENTS.len()].1;

        let mut swatches = div().flex().flex_row().gap_2();
        for (i, (label, hex)) in ACCENTS.iter().enumerate() {
            let v = view.clone();
            let selected = i == self.accent % ACCENTS.len();
            let mut sw = BaseButton::new(ElementId::Name(format!("group-accent-{i}").into()))
                .w(px(48.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .rounded(px(2.0))
                .border_1();
            sw = if selected {
                sw.border_color(theme::rgb(theme::colors::PRIMARY))
            } else {
                sw.border_color(theme::rgb(theme::colors::GRID_L1))
            };
            swatches = swatches.child(
                sw.on_click(move |_, _, cx: &mut App| {
                    v.update(cx, |p, cx| {
                        p.accent = i;
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .w_full()
                        .h(px(32.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme::rgb(*hex))
                        .child(if selected {
                            div().child(
                                svg()
                                    .path("icons/check.svg")
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                            )
                        } else {
                            div()
                        }),
                )
                .child(
                    div()
                        .font_family(theme::fonts::MONO)
                        .text_size(px(9.0))
                        .text_color(theme::rgb(theme::colors::MUTED))
                        .child(label.to_string()),
                ),
            );
        }

        let mut options = div().flex().flex_row().gap_3().w_full();
        for (i, opt) in PRIVACY.iter().enumerate() {
            let v = view.clone();
            let selected = i == self.privacy;
            let mut card = BaseButton::new(ElementId::Name(format!("group-privacy-{i}").into()))
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .rounded(px(theme::radius::DEFAULT))
                .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                .border_1();
            card = if selected {
                card.border_color(theme::rgb(theme::colors::PRIMARY))
            } else {
                card.border_color(theme::rgb(theme::colors::GRID_L1))
            };
            options = options.child(
                card.on_click(move |_, _, cx: &mut App| {
                    v.update(cx, |p, cx| {
                        p.privacy = i;
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .child(
                            svg()
                                .path(opt.icon)
                                .w(px(16.0))
                                .h(px(16.0))
                                .text_color(theme::rgb(theme::colors::PRIMARY)),
                        )
                        .child(if selected {
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(2.0))
                                .bg(theme::rgb(theme::colors::PRIMARY))
                                .child(
                                    svg()
                                        .path("icons/check.svg")
                                        .w(px(10.0))
                                        .h(px(10.0))
                                        .text_color(theme::rgb(
                                            theme::colors::ON_PRIMARY,
                                        )),
                                )
                        } else {
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .rounded(px(2.0))
                                .border_1()
                                .border_color(theme::rgb(theme::colors::GRID_L2))
                        }),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::rgb(theme::colors::INK))
                        .child(opt.title),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::rgb(theme::colors::MUTED))
                        .child(opt.desc),
                )
                .child(
                    div()
                        .font_family(theme::fonts::MONO)
                        .text_size(px(10.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::rgb(theme::colors::PRIMARY))
                        .child(opt.access),
                ),
            );
        }

        let mut perms = div().flex().flex_col().w_full();
        for (i, perm) in PERMS.iter().enumerate() {
            let v = view.clone();
            let on = self.perms[i];
            perms = perms.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .w_full()
                    .py_3()
                    .border_b_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .child(
                        svg()
                            .path(perm.icon)
                            .w(px(16.0))
                            .h(px(16.0))
                            .flex_shrink_0()
                            .text_color(theme::rgb(theme::colors::PRIMARY)),
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
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child(perm.title),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child(perm.desc),
                            ),
                    )
                    .child(
                        Switch::new(ElementId::Name(format!("group-perm-{i}").into()))
                            .checked(on)
                            .on_change(move |checked: &bool, _, cx: &mut App| {
                                let on = *checked;
                                v.update(cx, |p, cx| {
                                    p.perms[i] = on;
                                    cx.notify();
                                });
                            }),
                    ),
            );
        }

        let cv = view.clone();
        let back = self.on_back.clone();
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
                    .id(ElementId::Name("group-form-scroll".into()))
                    .overflow_y_scroll()
                    .p_5()
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
                                                            .child("GROUPS / NEW GROUP SPEC"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .font_family(theme::fonts::HEADLINE)
                                                    .text_size(px(22.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::rgb(
                                                        theme::colors::INK,
                                                    ))
                                                    .child("Create New Group"),
                                            ),
                                    )
                                    .child(
                                        BaseButton::new(ElementId::Name("group-create".into()))
                                            .h(px(34.0))
                                            .px_4()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_center()
                                            .gap_2()
                                            .rounded(px(theme::radius::DEFAULT))
                                            .bg(theme::rgb(theme_accent))
                                            .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::BOLD)
                                            .on_click(move |_, _, cx: &mut App| {
                                                cv.update(cx, |p, cx| p.try_create(cx));
                                            })
                                            .child(
                                                svg()
                                                    .path("icons/check.svg")
                                                    .w(px(14.0))
                                                    .h(px(14.0))
                                                    .text_color(theme::rgb(
                                                        theme::colors::ON_PRIMARY,
                                                    )),
                                            )
                                            .child("Create Group"),
                                    ),
                            )
                            .child({
                                let bv = view.clone();
                                MonoLink::new("Back to chat").on_click(
                                    move |_, window, cx: &mut App| {
                                        if let Some(back) = back.clone() {
                                            back(window, cx);
                                        } else {
                                            bv.update(cx, |_, _| {});
                                        }
                                    },
                                )
                            }),
                    )
                    .child({
                        let notice: AnyElement = if let Some(error) = self.error.clone() {
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::rgb(theme::colors::ERROR))
                                .child(error)
                                .into_any_element()
                        } else if self.created {
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::rgb(theme::colors::SUCCESS))
                                .child(SharedString::from(format!(
                                    "Group spec ready — {} member{} · #{}",
                                    self.added.len(),
                                    if self.added.len() == 1 { "" } else { "s" },
                                    slug
                                )))
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        };
                        notice
                    })
                    .child(
                        self.render_card(
                            "Group Profile & Aesthetics",
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .w_full()
                                .child(
                                    div()
                                        .w(px(72.0))
                                        .h(px(72.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(theme::radius::DEFAULT))
                                        .bg(theme::rgb(theme::colors::SURFACE_LOW))
                                        .border_1()
                                        .border_color(theme::rgb(theme::colors::GRID_L1))
                                        .child(
                                            svg()
                                                .path("icons/photo.svg")
                                                .w(px(24.0))
                                                .h(px(24.0))
                                                .text_color(theme::rgb(
                                                    theme::colors::MUTED,
                                                )),
                                        ),
                                )
                                .child(FormField::new("GROUP NAME", self.name.clone()))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .font_family(theme::fonts::MONO)
                                                .text_size(px(10.0))
                                                .text_color(theme::rgb(
                                                    theme::colors::MUTED,
                                                ))
                                                .child("Canonical Chat Slug:"),
                                        )
                                        .child(
                                            div()
                                                .px_1()
                                                .rounded(px(2.0))
                                                .bg(theme::hsla(theme::colors::PRIMARY, 0.12))
                                                .border_1()
                                                .border_color(theme::hsla(
                                                    theme::colors::PRIMARY,
                                                    0.4,
                                                ))
                                                .font_family(theme::fonts::MONO)
                                                .text_size(px(11.0))
                                                .text_color(theme::rgb(
                                                    theme::colors::PRIMARY,
                                                ))
                                                .child(SharedString::from(format!("#{slug}"))),
                                        ),
                                )
                                .child(
                                    FormField::new("DESCRIPTION", self.description.clone())
                                        .header_extra(
                                            div()
                                                .font_family(theme::fonts::MONO)
                                                .text_size(px(9.0))
                                                .text_color(theme::rgb(theme::colors::MUTED))
                                                .child(SharedString::from(format!(
                                                    "{} / 200",
                                                    desc_len.min(200)
                                                ))),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .w_full()
                                        .child(
                                            div()
                                                .font_family(theme::fonts::MONO)
                                                .text_size(px(10.0))
                                                .text_color(theme::rgb(
                                                    theme::colors::MUTED,
                                                ))
                                                .child("CHAT THEME ACCENT"),
                                        )
                                        .child(swatches),
                                ),
                        ),
                    )
                    .child(
                        self.render_card(
                            "Privacy & Access",
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .w_full()
                                .child(options),
                        ),
                    )
                    .child(self.render_card("Media & Social Permissions", perms)),
            )
    }
}

/// Inspector sheet content: member management for the open group form.
/// Holds the form entity so both sides stay live.
pub struct GroupMembersSheet {
    form: Entity<GroupForm>,
    _watches: Vec<Subscription>,
}

impl GroupMembersSheet {
    pub fn new(form: Entity<GroupForm>, cx: &mut Context<Self>) -> Self {
        let watches = vec![cx.observe(&form, |_, _, cx| cx.notify())];
        Self {
            form,
            _watches: watches,
        }
    }

    fn invite_link(&self, cx: &mut Context<Self>) -> String {
        let name = self.form.read(cx).name.clone();
        let slug = slugify(&name.read(cx).value());
        format!("varta.gg/invite/{slug}")
    }
}

impl Render for GroupMembersSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let (members, copied) = self.form.read_with(cx, |form, _| {
            (form.members().to_vec(), form.copied)
        });
        let link = self.invite_link(cx);

        let mut added = div().flex().flex_col().w_full();
        for m in &members {
            let rv = view.clone();
            let handle = m.handle.clone();
            added = added.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .py_1()
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(theme::rgb(theme::colors::SURFACE_HIGH))
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::rgb(theme::colors::PRIMARY_DARK))
                            .child(m.initials.clone()),
                    )
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::INK))
                            .child(m.handle.clone()),
                    )
                    .child(
                        div().ml_auto().child(
                            BaseButton::new(ElementId::Name(
                                format!("group-remove-{handle}").into(),
                            ))
                            .w(px(20.0))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(2.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .on_click(move |_, _, cx: &mut App| {
                                let h = handle.clone();
                                rv.update(cx, |s, cx| {
                                    s.form.update(cx, |f, cx| f.remove_member(&h, cx));
                                });
                            })
                            .child(
                                svg()
                                    .path("icons/x-mark.svg")
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .text_color(theme::rgb(theme::colors::MUTED)),
                            ),
                        ),
                    ),
            );
        }

        let mut suggested = div().flex().flex_col().w_full();
        for s in SUGGESTED {
            let av = view.clone();
            let already = members.iter().any(|m| m.handle == s.handle);
            let handle = s.handle;
            suggested = suggested.child(
                BaseButton::new(ElementId::Name(format!("group-suggest-{handle}").into()))
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .py_2()
                    .border_b_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .text_color(theme::rgb(theme::colors::INK))
                    .on_click(move |_, _, cx: &mut App| {
                        av.update(cx, |sheet, cx| {
                            sheet.form.update(cx, |f, cx| f.add_handle(handle, cx));
                        });
                    })
                    .child(
                        div()
                            .relative()
                            .w(px(36.0))
                            .h(px(36.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(theme::rgb(theme::colors::SURFACE_HIGH))
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::rgb(theme::colors::PRIMARY_DARK))
                            .child(s.initials)
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(0.0))
                                    .right(px(0.0))
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .rounded_full()
                                    .bg(theme::rgb(s.presence))
                                    .border_1()
                                    .border_color(theme::rgb(0xffffff)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child(s.name),
                            )
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(10.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child(s.handle),
                            ),
                    )
                    .child(if already {
                        div().ml_auto().child(
                            svg()
                                .path("icons/check.svg")
                                .w(px(14.0))
                                .h(px(14.0))
                                .text_color(theme::rgb(theme::colors::SUCCESS)),
                        )
                    } else {
                        div()
                    }),
            );
        }

        let cv = view.clone();
        let link_copy = link.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .p_4()
            .on_action(|_: &Cancel, window, cx| window.close_sheet(cx))
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
                            .child("Add People by Username"),
                    )
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(10.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child(SharedString::from(format!(
                                "{} Added",
                                members.len()
                            ))),
                    ),
            )
            .child({
                let form = self.form.clone();
                let fv = view.clone();
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
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("Search or Enter Handle"),
                            )
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .text_color(theme::rgb(theme::colors::PRIMARY))
                                    .child("Press Enter to add"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(gpui_kit::base::input::Input::new(&form.read(cx).member_search)),
                            )
                            .child(
                                BaseButton::new(ElementId::Name("group-add".into()))
                                    .h(px(36.0))
                                    .px_3()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .gap_1()
                                    .rounded(px(theme::radius::DEFAULT))
                                    .bg(theme::rgb(theme::colors::PRIMARY))
                                    .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::BOLD)
                                    .on_click(move |_, _, cx: &mut App| {
                                        fv.update(cx, |s, cx| {
                                            s.form.update(cx, |f, cx| f.add_member(cx));
                                        });
                                    })
                                    .child(
                                        svg()
                                            .path("icons/user-plus.svg")
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .text_color(theme::rgb(
                                                theme::colors::ON_PRIMARY,
                                            )),
                                    )
                                    .child("Add"),
                            ),
                    )
            })
            .child(
                div()
                    .font_family(theme::fonts::MONO)
                    .text_size(px(9.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child("Suggested Friends"),
            )
            .child(suggested)
            .child(added)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .w_full()
                    .mt_2()
                    .p_3()
                    .rounded(px(theme::radius::DEFAULT))
                    .bg(theme::hsla(theme::colors::PRIMARY, 0.05))
                    .border_1()
                    .border_color(theme::hsla(theme::colors::PRIMARY, 0.35))
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::INK))
                            .child("Share Invite Link"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .child(
                                svg()
                                    .path("icons/link.svg")
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .flex_shrink_0()
                                    .text_color(theme::rgb(theme::colors::PRIMARY)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(10.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child(
                                        "Anyone with this link can request access",
                                    ),
                            )
                            .child(
                                BaseButton::new(ElementId::Name("group-copy-link".into()))
                                    .h(px(30.0))
                                    .px_2()
                                    .flex_shrink_0()
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
                                    .text_size(px(10.0))
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .on_click(move |_, _, cx: &mut App| {
                                        cv.update(cx, |s, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                link_copy.clone(),
                                            ));
                                            s.form.update(cx, |f, cx| {
                                                f.copied = true;
                                                cx.notify();
                                            });
                                        });
                                    })
                                    .child(if copied { "Copied" } else { "Copy Link" }),
                            ),
                    )
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(10.0))
                            .text_color(theme::rgb(theme::colors::PRIMARY))
                            .child(SharedString::from(link)),
                    ),
            )
    }
}
