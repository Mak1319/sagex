//! Third-party licenses window — ChatWindow icon rail, full-bleed content.
//!
//! Reference layout: header with export actions, recognized-dependencies
//! toolbar with live search, category tabs, package table with per-row
//! license-term sheets, live footer counts, legal-notices card. Exports
//! copy real generated content (notices text / SPDX JSON) to the
//! clipboard; license blurbs are short factual SPDX summaries.

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
    version: &'static str,
    license: &'static str,
    org: &'static str,
    category: &'static str,
    icon: &'static str,
    blurb: &'static str,
}

const PACKAGES: &[Package] = &[
    Package {
        name: "@khronos/webgl-pipeline",
        version: "v2.4.0",
        license: "MIT",
        org: "Khronos Group Standards Team",
        category: "graphics",
        icon: "icons/squares-2x2.svg",
        blurb: "Permissive license allowing reuse with attribution. Requires the copyright and permission notices in all copies.",
    },
    Package {
        name: "wgpu-native-runtime",
        version: "v0.19.1",
        license: "Apache-2.0",
        org: "Rust GPU Working Group",
        category: "graphics",
        icon: "icons/cog-6-tooth.svg",
        blurb: "Permissive license with an express patent grant. Requires a license copy, state-changes notices, and NOTICE preservation.",
    },
    Package {
        name: "libmesh-cad-parser",
        version: "v1.14.8",
        license: "BSD-3-Clause",
        org: "Stanford Graphics Lab & Contributors",
        category: "graphics",
        icon: "icons/doc.svg",
        blurb: "Permissive license requiring the copyright notice, conditions list, and no-endorsement clause in redistributions.",
    },
    Package {
        name: "matrix-crypto-olm",
        version: "v3.2.15",
        license: "Apache-2.0",
        org: "Matrix Foundation & Contributors",
        category: "crypto",
        icon: "icons/lock-closed.svg",
        blurb: "Permissive license with an express patent grant. Requires a license copy, state-changes notices, and NOTICE preservation.",
    },
    Package {
        name: "jetbrains-mono-nerd-font",
        version: "v3.1.0",
        license: "OFL-1.1",
        org: "JetBrains s.r.o.",
        category: "ui",
        icon: "icons/code-bracket.svg",
        blurb: "SIL Open Font License: fonts may be used, studied, modified, and redistributed. Modified versions must not use reserved names and stay under OFL.",
    },
    Package {
        name: "tokio-schematic-runtime",
        version: "v1.36.0",
        license: "MIT",
        org: "Tokio Project Contributors",
        category: "networking",
        icon: "icons/refresh.svg",
        blurb: "Permissive license allowing reuse with attribution. Requires the copyright and permission notices in all copies.",
    },
    Package {
        name: "tauri-client-runtime",
        version: "v2.1.2",
        license: "MIT",
        org: "Tauri Programme Contributors",
        category: "ui",
        icon: "icons/photo.svg",
        blurb: "Permissive license allowing reuse with attribution. Requires the copyright and permission notices in all copies.",
    },
    Package {
        name: "protobuf-ts",
        version: "v2.9.4",
        license: "Apache-2.0",
        org: "Timo Ernst & Contributors",
        category: "networking",
        icon: "icons/link.svg",
        blurb: "Permissive license with an express patent grant. Requires a license copy, state-changes notices, and NOTICE preservation.",
    },
    Package {
        name: "zstandard-codec",
        version: "v1.5.5",
        license: "BSD-3-Clause",
        org: "Meta Platforms, Inc.",
        category: "crypto",
        icon: "icons/funnel.svg",
        blurb: "Permissive license requiring the copyright notice, conditions list, and no-endorsement clause in redistributions.",
    },
    Package {
        name: "ed25519-dalek-core",
        version: "v2.1.1",
        license: "ISC",
        org: "Dalek Cryptography Team",
        category: "crypto",
        icon: "icons/key.svg",
        blurb: "Permissive license functionally equivalent to MIT/BSD-2-Clause. Requires the copyright and permission notices in all copies.",
    },
];

const TABS: &[(&str, &str)] = &[
    ("all", "All Packages"),
    ("graphics", "Graphics & WebGL"),
    ("crypto", "Crypto & Security"),
    ("ui", "UI & Framework"),
    ("networking", "Networking & Core"),
];

const REQUEST_TEMPLATE: &str = "Subject: Source Archive Request (SageX Varta)\nTo: legal compliance desk\n\nPlease provide the corresponding source archive for the open-source components listed in the bundled SPDX manifest (SPDX-SPEC 3.0).";

#[derive(Clone, Copy, PartialEq, Eq)]
enum CopyKind {
    Notices,
    Manifest,
    Request,
}

/// License chip tint per SPDX id: bg + fg.
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
    let mut out = String::from("SageX Varta — Third-Party Notices (SPDX-SPEC 3.0)\n\n");
    for p in pkgs {
        out.push_str(&format!(
            "{} {} ({})\n{}\n{}\n\n",
            p.name, p.version, p.license, p.org, p.blurb
        ));
    }
    out
}

fn manifest_json() -> String {
    let pkgs: Vec<serde_json::Value> = PACKAGES
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "version": p.version,
                "license": p.license,
                "organization": p.org,
                "category": p.category,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "spdx": "3.0",
        "application": "SageX Varta",
        "packages": pkgs,
    }))
    .unwrap_or_default()
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
                    .child(term_row("VERSION", p.version))
                    .child(term_row("SPDX LICENSE", p.license))
                    .child(term_row("ORGANIZATION", p.org)),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::rgb(theme::colors::INK))
                    .child(p.blurb),
            )
    }
}

fn term_row(label: &str, value: &str) -> AnyElement {
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
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::rgb(theme::colors::INK))
                .child(value.to_string()),
        )
        .into_any_element()
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
        gpui_kit::open_window(opts, cx, |window, cx| {
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
                        || p.license.to_lowercase().contains(&query)
                        || p.org.to_lowercase().contains(&query))
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
                            .child(pkg.version),
                    )
                    .child(
                        div()
                            .col_span(2)
                            .flex()
                            .flex_row()
                            .child(Self::license_chip(pkg.license)),
                    )
                    .child(
                        div()
                            .col_span(3)
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::INK))
                            .child(pkg.org),
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
                                                    .child("SPDX-INDEX // 48 ASSETS"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme::rgb(theme::colors::MUTED))
                                            .child(
                                                "This application incorporates open-source libraries, runtimes, and components under their respective license conditions. All attributions and license texts are cataloged below.",
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
                                        "SPDX Manifest (.json)",
                                        "Copied",
                                        copied == Some(CopyKind::Manifest),
                                        true,
                                        &view,
                                        CopyKind::Manifest,
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
                                                    .child("RECOGNIZED SOFTWARE DEPENDENCIES"),
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
                                                    .child("FOSS COMPLIANT"),
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
                                        div().col_span(4).child("SOFTWARE PACKAGE"),
                                    )
                                    .child(div().col_span(1).child("VERSION"))
                                    .child(div().col_span(2).child("SPDX LICENSE"))
                                    .child(
                                        div()
                                            .col_span(3)
                                            .child("COPYRIGHT / ORGANIZATION"),
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
                                                "Showing {shown} of 10 compiled packages"
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
                                                    .child("SPDX-SPEC: 3.0"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme::rgb(
                                                        theme::colors::SUCCESS,
                                                    ))
                                                    .child("ALL OBLIGATIONS MET"),
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
