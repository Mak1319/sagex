//! Third-party licenses window — ChatWindow icon rail, full-bleed content.
//!
//! Reference layout: header with export actions, recognized-dependencies
//! toolbar with live search, category tabs, package table with per-row
//! license-term sheets, live footer counts, legal-notices card. Exports
//! copy the checked-in license inventory and notices to the clipboard.

use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::input::{Input, InputState as KitInput};
use gpui_kit::component::{Placement, WindowExt};
use gpui_kit::*;

use crate::layout::icon_rail::IconRail;
use crate::theme;

/// Locked initial size of the licenses window. Resizable like chat.
pub const LICENSE_WINDOW_SIZE: [f32; 2] = [1100.0, 750.0];

struct Package {
    name: &'static str,
    version: Option<&'static str>,
    license: Option<&'static str>,
    author: Option<&'static str>,
    category: &'static str,
    icon: &'static str,
    description: Option<&'static str>,
}

include!(concat!(env!("OUT_DIR"), "/licenses_data.rs"));

const TABS: &[(&str, &str)] = &[
    ("all", "All Items"),
    ("graphics", "Graphics & WebGL"),
    ("crypto", "Crypto & Security"),
    ("ui", "UI & Framework"),
    ("networking", "Networking & Core"),
];

const REQUEST_TEMPLATE: &str = "Subject: Source Archive Request (SageX Varta)\nTo: legal compliance desk\n\nPlease provide the corresponding source archive for the open-source components listed in the bundled license inventory.";

#[derive(Clone, Copy, PartialEq, Eq)]
enum CopyKind {
    Notices,
    Inventory,
    Request,
}

/// License chip tint by common license identifier: background + foreground.
fn license_tint(id: &str) -> (u32, u32) {
    match id {
        "MIT" => (0xdbeafe, theme::colors::PRIMARY_DARK),
        "Apache-2.0" => (0xe2e8f0, 0x334155),
        "BSD-3-Clause" => (0xe2e8f0, 0x334155),
        "OFL-1.1" => (0xccfbf1, 0x0f766e),
        _ => (0xd1fae5, theme::colors::SUCCESS),
    }
}

fn notices_txt(pkgs: &[&Package]) -> String {
    let mut out = String::from("SageX Varta — License Inventory Notices\n\n");
    for p in pkgs {
        out.push_str(&format!(
            "{} {} ({})\n{}\n{}\n\n",
            p.name,
            p.version.unwrap_or("Version not recorded"),
            p.license.unwrap_or("License not recorded"),
            p.author.unwrap_or("Author not recorded"),
            p.description.unwrap_or("No description recorded.")
        ));
    }
    out
}

fn manifest_json() -> String {
    INVENTORY_JSON.to_string()
}

#[derive(IntoElement)]
struct LicenseSheet {
    pkg: &'static Package,
}

impl RenderOnce for LicenseSheet {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = self.pkg;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .p_4()
            .child(
                div()
                    .font_family(theme::fonts::MONO)
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::rgb(theme::colors::INK))
                    .child(p.name),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .w_full()
                    .child(TermRow::new("VERSION", p.version.unwrap_or("Not recorded")))
                    .child(TermRow::new("LICENSE", p.license.unwrap_or("Not recorded")))
                    .child(TermRow::new("AUTHOR", p.author.unwrap_or("Not recorded"))),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::rgb(theme::colors::INK))
                    .child(p.description.unwrap_or("No description recorded.")),
            )
    }
}

/// Term row — RenderOnce so no free function builds component trees.
#[derive(IntoElement)]
struct TermRow {
    label: SharedString,
    value: SharedString,
}

impl TermRow {
    fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

impl RenderOnce for TermRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
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
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child(self.label),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::rgb(theme::colors::INK))
                    .child(self.value),
            )
    }
}

fn open_terms(pkg: &'static Package, window: &mut Window, cx: &mut App) {
    window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
        sheet
            .title(pkg.name)
            .size(px(360.0))
            .resizable(true)
            .overlay(false)
            .overlay_closable(false)
            .child(LicenseSheet { pkg })
    });
}

pub struct LicensePage {
    theme: u8,
    search: Entity<KitInput>,
    tab: &'static str,
    copied: Option<CopyKind>,
    _watches: Vec<Subscription>,
}

impl LicensePage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            KitInput::new(window, cx).placeholder("Search by package, license, or author…")
        });
        let _watches = vec![cx.observe(&search, |_, _, cx| cx.notify())];
        Self {
            theme: 1,
            search,
            tab: "all",
            copied: None,
            _watches,
        }
    }

    pub fn open(cx: &mut App) -> bool {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(
                    px(LICENSE_WINDOW_SIZE[0]),
                    px(LICENSE_WINDOW_SIZE[1]),
                ),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Licenses".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        gpui_kit::open_window(crate::assets::with_app_id(opts), cx, |window, cx| {
            cx.new(|cx| LicensePage::new(window, cx))
        })
        .is_ok()
    }

    fn set_theme(&mut self, index: u8, cx: &mut Context<Self>) {
        self.theme = index.min(2);
        cx.notify();
    }

    fn set_tab(&mut self, tab: &'static str, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    fn copy(&mut self, kind: CopyKind, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.copied = Some(kind);
        cx.notify();
    }

    fn visible(&self, cx: &mut Context<Self>) -> Vec<&'static Package> {
        let query = self.search.read(cx).value().to_lowercase();
        PACKAGES
            .iter()
            .filter(|p| {
                (self.tab == "all" || p.category == self.tab)
                    && (query.is_empty()
                        || p.name.to_lowercase().contains(&query)
                        || p.license.unwrap_or("").to_lowercase().contains(&query)
                        || p.author.unwrap_or("").to_lowercase().contains(&query))
            })
            .collect()
    }

    fn export_button(
        id: &str,
        label: &'static str,
        active_label: &'static str,
        is_copied: bool,
        primary: bool,
        view: &Entity<LicensePage>,
        kind: CopyKind,
        text: String,
    ) -> BaseButton {
        let v = view.clone();
        let mut btn = BaseButton::new(ElementId::Name(id.into()))
            .h(px(34.0))
            .px_3()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap_2()
            .rounded(px(theme::radius::DEFAULT))
            .font_family(theme::fonts::MONO)
            .text_size(px(11.0))
            .font_weight(FontWeight::MEDIUM);
        btn = if primary {
            btn.bg(theme::rgb(theme::colors::PRIMARY))
                .text_color(theme::rgb(theme::colors::ON_PRIMARY))
        } else {
            btn.bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                .border_1()
                .border_color(theme::rgb(theme::colors::GRID_L2))
                .text_color(theme::rgb(theme::colors::INK))
        };
        btn.on_click(move |_, _, cx: &mut App| {
            v.update(cx, |p, cx| p.copy(kind, text.clone(), cx));
        })
        .child(if is_copied { active_label } else { label })
    }

    fn terms_button(pkg: &'static Package) -> BaseButton {
        BaseButton::new(ElementId::Name(
            format!("license-terms-{}", pkg.name).into(),
        ))
        .col_span(2)
        .h(px(28.0))
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::radius::DEFAULT))
        .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
        .border_1()
        .border_color(theme::rgb(theme::colors::GRID_L2))
        .font_family(theme::fonts::MONO)
        .text_size(px(10.0))
        .text_color(theme::rgb(theme::colors::INK))
        .on_click(move |_, window, cx: &mut App| {
            open_terms(pkg, window, cx);
        })
        .child("View License Terms")
    }

    fn license_chip(id: &str) -> AnyElement {
        let (bg, fg) = license_tint(id);
        div()
            .px_1()
            .rounded(px(2.0))
            .bg(theme::rgb(bg))
            .font_family(theme::fonts::MONO)
            .text_size(px(9.0))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::rgb(fg))
            .child(id.to_string())
            .into_any_element()
    }
}

impl Render for LicensePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let view = cx.entity().clone();
        let rows = self.visible(cx);
        let shown = rows.len();
        let copied = self.copied;

        let mut tabs = div().flex().flex_row().gap_2();
        for (id, label) in TABS {
            let count = if *id == "all" {
                PACKAGES.len()
            } else {
                PACKAGES.iter().filter(|p| p.category == *id).count()
            };
            let v = view.clone();
            let mut tab = BaseButton::new(ElementId::Name(format!("license-tab-{id}").into()))
                .h(px(28.0))
                .px_3()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::radius::DEFAULT))
                .font_family(theme::fonts::MONO)
                .text_size(px(10.0))
                .font_weight(FontWeight::MEDIUM);
            tab = if self.tab == *id {
                tab.bg(theme::rgb(theme::colors::PRIMARY))
                    .text_color(theme::rgb(theme::colors::ON_PRIMARY))
            } else {
                tab.bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L2))
                    .text_color(theme::rgb(theme::colors::MUTED))
            };
            tabs = tabs.child(
                tab.on_click(move |_, _, cx: &mut App| {
                    v.update(cx, |p, cx| p.set_tab(id, cx));
                })
                .child(SharedString::from(format!("{label} ({count})"))),
            );
        }

        let mut table = div().flex().flex_col().w_full();
        for pkg in rows {
            table = table.child(
                div()
                    .grid()
                    .grid_cols(12)
                    .items_center()
                    .gap_3()
                    .w_full()
                    .py_2()
                    .px_4()
                    .border_b_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .child(
                        div()
                            .col_span(4)
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(
                                svg()
                                    .path(pkg.icon)
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .flex_shrink_0()
                                    .text_color(theme::rgb(theme::colors::PRIMARY)),
                            )
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child(pkg.name),
                            ),
                    )
                    .child(
                        div()
                            .col_span(1)
                            .font_family(theme::fonts::MONO)
                            .text_size(px(11.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child(pkg.version.unwrap_or("—")),
                    )
                    .child(
                        div()
                            .col_span(2)
                            .flex()
                            .flex_row()
                            .child(Self::license_chip(pkg.license.unwrap_or("Unknown"))),
                    )
                    .child(
                        div()
                            .col_span(3)
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::INK))
                            .child(pkg.author.unwrap_or("Not recorded")),
                    )
                    .child(
                        Self::terms_button(pkg),
                    ),
            );
        }

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(theme::rgb(theme::colors::BACKGROUND))
            .child(
                IconRail::new()
                    .theme(theme)
                    .on_theme({
                        let v = view.clone();
                        move |index, _window, cx: &mut App| {
                            v.update(cx, |p, cx| p.set_theme(index, cx));
                        }
                    })
                    .on_settings(|_, _, _| {}),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .id(ElementId::Name("license-scroll".into()))
                    .overflow_y_scroll()
                    .px_6()
                    .py_5()
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
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                svg()
                                                    .path("icons/shield-check.svg")
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
                                                    .child("OPEN SOURCE COMPLIANCE"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_baseline()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .font_family(theme::fonts::HEADLINE)
                                                    .text_size(px(22.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::rgb(
                                                        theme::colors::INK,
                                                    ))
                                                    .child("Third-Party Software & Licenses"),
                                            )
                                            .child(
                                                div()
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(10.0))
                                                    .text_color(theme::rgb(
                                                        theme::colors::MUTED,
                                                    ))
                                                    .child(format!("LICENSE INVENTORY // {} ITEMS", PACKAGES.len())),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme::rgb(theme::colors::MUTED))
                                            .child(
                                                "This inventory records direct dependencies and bundled assets. Missing license or provenance details are marked as not recorded.",
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .flex_shrink_0()
                                    .child(Self::export_button(
                                        "license-export-txt",
                                        "Export Notices (.txt)",
                                        "Copied",
                                        copied == Some(CopyKind::Notices),
                                        false,
                                        &view,
                                        CopyKind::Notices,
                                        notices_txt(&self.visible(cx)),
                                    ))
                                    .child(Self::export_button(
                                        "license-export-json",
                                        "License Inventory (.json)",
                                        "Copied",
                                        copied == Some(CopyKind::Inventory),
                                        true,
                                        &view,
                                        CopyKind::Inventory,
                                        manifest_json(),
                                    )),
                            ),
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
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .w_full()
                                    .px_4()
                                    .py_3()
                                    .border_b_1()
                                    .border_color(theme::rgb(theme::colors::GRID_L1))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                svg()
                                                    .path("icons/slides.svg")
                                                    .w(px(14.0))
                                                    .h(px(14.0))
                                                    .text_color(theme::rgb(
                                                        theme::colors::PRIMARY,
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(10.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme::rgb(
                                                        theme::colors::INK,
                                                    ))
                                            .child("INVENTORY ITEMS"),
                                            )
                                            .child(
                                                div()
                                                    .px_1()
                                                    .rounded(px(2.0))
                                                    .bg(theme::hsla(theme::colors::SUCCESS, 0.14))
                                                    .font_family(theme::fonts::MONO)
                                                    .text_size(px(9.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme::rgb(
                                                        theme::colors::SUCCESS,
                                                    ))
                                                    .child("MANUALLY MAINTAINED"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .w(px(300.0))
                                            .h(px(32.0))
                                            .px_3()
                                            .rounded(px(theme::radius::DEFAULT))
                                            .border_1()
                                            .border_color(theme::rgb(theme::colors::GRID_L1))
                                            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                                            .child(
                                                svg()
                                                    .path("icons/magnifying-glass.svg")
                                                    .w(px(14.0))
                                                    .h(px(14.0))
                                                    .flex_shrink_0()
                                                    .text_color(theme::rgb(
                                                        theme::colors::MUTED,
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_size(px(12.0))
                                                    .text_color(theme::rgb(
                                                        theme::colors::INK,
                                                    ))
                                                    .child(Input::new(&self.search)),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .w_full()
                                    .px_4()
                                    .py_2()
                                    .border_b_1()
                                    .border_color(theme::rgb(theme::colors::GRID_L1))
                                    .child(tabs),
                            )
                            .child(
                                div()
                                    .grid()
                                    .grid_cols(12)
                                    .gap_3()
                                    .w_full()
                                    .px_4()
                                    .py_2()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(9.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child(
                                        div().col_span(4).child("INVENTORY ITEM"),
                                    )
                                    .child(div().col_span(1).child("VERSION"))
                                    .child(div().col_span(2).child("LICENSE"))
                                    .child(
                                        div()
                                            .col_span(3)
                                            .child("AUTHOR"),
                                    )
                                    .child(div().col_span(2).child("ACTION")),
                            )
                            .child(table)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .w_full()
                                    .px_4()
                                    .py_2()
                                    .child(
                                        div()
                                            .font_family(theme::fonts::MONO)
                                            .text_size(px(9.0))
                                            .text_color(theme::rgb(theme::colors::MUTED))
                                            .child(SharedString::from(format!(
                                                "Showing {shown} of {} inventory items",
                                                PACKAGES.len()
                                            ))),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_2()
                                            .font_family(theme::fonts::MONO)
                                            .text_size(px(9.0))
                                            .child(
                                                div()
                                                    .text_color(theme::rgb(
                                                        theme::colors::MUTED,
                                                    ))
                                                    .child("MANUAL INVENTORY"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme::rgb(
                                                        theme::colors::SUCCESS,
                                                    ))
                                                    .child("METADATA MAY BE INCOMPLETE"),
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
                            .mt_4()
                            .rounded(px(theme::radius::DEFAULT))
                            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                            .border_1()
                            .border_color(theme::rgb(theme::colors::GRID_L1))
                            .p_4()
                            .child(
                                div()
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(theme::radius::DEFAULT))
                                    .border_1()
                                    .border_color(theme::rgb(theme::colors::GRID_L1))
                                    .child(
                                        svg()
                                            .path("icons/shield-check.svg")
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .text_color(theme::rgb(theme::colors::MUTED)),
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
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::rgb(theme::colors::INK))
                                            .child("Legal Notices & Source Code Requests"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(theme::rgb(theme::colors::MUTED))
                                            .child(
                                                "This software application incorporates third-party open-source components provided \"as is\" under their respective licenses. Where applicable license terms grant you rights to the corresponding source code, you may request an archive from our legal compliance desk.",
                                            ),
                                    ),
                            )
                            .child(Self::export_button(
                                "license-request-src",
                                "Request Source Archive",
                                "Copied",
                                copied == Some(CopyKind::Request),
                                false,
                                &view,
                                CopyKind::Request,
                                REQUEST_TEMPLATE.to_string(),
                            )),
                    ),
            )
    }
}
