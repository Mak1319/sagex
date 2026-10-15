//! Session passphrase dialog — standalone route window.
//!
//! Frameless modal mirroring the reference unlock dialog: info banner,
//! passphrase field with visibility toggle and live entropy meter,
//! Secure Enclave cache checkbox, hardware-token alternative, and
//! Cancel / Authenticate actions. Every control is live; Authenticate
//! validates locally (real unlock wiring arrives with the backend).

use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::*;

use crate::components::deck_checkbox::DeckCheckbox;
use crate::components::form_field::FormField;
use crate::theme;

/// Corner registration mark: L-shaped bracket, blueprint spec (same
/// convention as the login/signup pages).
#[derive(Clone, Copy)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(IntoElement)]
struct CornerMark {
    corner: Corner,
}

impl CornerMark {
    fn new(corner: Corner) -> Self {
        Self { corner }
    }
}

impl RenderOnce for CornerMark {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let bar = theme::hsla(theme::colors::PRIMARY, 0.4);
        let right = matches!(self.corner, Corner::TopRight | Corner::BottomRight);
        let bottom = matches!(self.corner, Corner::BottomLeft | Corner::BottomRight);
        let mut h = div().absolute().w(px(16.0)).h(px(2.0)).bg(bar);
        h = if bottom {
            h.top(px(14.0)).left(px(0.0))
        } else {
            h.top(px(0.0)).left(px(0.0))
        };
        let mut v = div().absolute().w(px(2.0)).h(px(16.0)).bg(bar);
        v = if right {
            v.top(px(0.0)).left(px(14.0))
        } else {
            v.top(px(0.0)).left(px(0.0))
        };
        div()
            .relative()
            .w(px(16.0))
            .h(px(16.0))
            .flex_shrink_0()
            .child(h)
            .child(v)
    }
}

/// Locked content size of the dialog. Single source for the `main.rs`
/// bounds/min-size.
pub const PASSPHRASE_WINDOW_SIZE: [f32; 2] = [500.0, 410.0];

/// 0..=4 strength segments from length + character classes.
fn entropy_score(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let len = text.chars().count();
    let mut classes = 0;
    if text.bytes().any(|b| b.is_ascii_lowercase()) {
        classes += 1;
    }
    if text.bytes().any(|b| b.is_ascii_uppercase()) {
        classes += 1;
    }
    if text.bytes().any(|b| b.is_ascii_digit()) {
        classes += 1;
    }
    if text.bytes().any(|b| !b.is_ascii_alphanumeric()) {
        classes += 1;
    }
    let mut score = 1;
    if len >= 8 {
        score += 1;
    }
    if len >= 14 && classes >= 2 {
        score += 1;
    }
    if len >= 20 && classes >= 3 {
        score += 1;
    }
    score.min(4)
}

pub struct PassphraseDialog {
    passphrase: Entity<InputState>,
    show_passphrase: bool,
    cache_key: bool,
    hw_mode: bool,
    target_pinned: bool,
    error: Option<SharedString>,
    _watches: Vec<Subscription>,
}

impl PassphraseDialog {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let passphrase = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Enter session passphrase")
        });
        let mut _watches = vec![cx.observe(
            &passphrase,
            |view: &mut PassphraseDialog,
             _: Entity<InputState>,
             cx: &mut Context<PassphraseDialog>| {
                view.error = None;
                cx.notify();
            },
        )];
        _watches.push(cx.subscribe(
            &passphrase,
            |view: &mut PassphraseDialog,
             _: Entity<InputState>,
             event: &InputEvent,
             cx: &mut Context<PassphraseDialog>| {
                if matches!(event, InputEvent::PressEnter { .. })
                    && !view.try_authenticate(cx)
                {
                    // Validation failed; the inline error is already shown.
                }
            },
        ));
        Self {
            passphrase,
            show_passphrase: true,
            cache_key: true,
            hw_mode: false,
            target_pinned: false,
            error: None,
            _watches,
        }
    }

    pub fn open(cx: &mut App) -> bool {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(
                    px(PASSPHRASE_WINDOW_SIZE[0]),
                    px(PASSPHRASE_WINDOW_SIZE[1]),
                ),
                cx,
            ))),
            titlebar: None,
            window_decorations: Some(WindowDecorations::Client),
            is_resizable: false,
            window_min_size: Some(size(
                px(PASSPHRASE_WINDOW_SIZE[0]),
                px(PASSPHRASE_WINDOW_SIZE[1]),
            )),
            ..Default::default()
        };
        gpui_kit::open_window(opts, cx, |window, cx| {
            cx.new(|cx| PassphraseDialog::new(window, cx))
        })
        .is_ok()
    }

    /// Local validation: hardware-token mode needs no secret, otherwise
    /// the field must be non-empty. True = dialog accepted.
    fn try_authenticate(&mut self, cx: &mut Context<Self>) -> bool {
        if self.hw_mode {
            return true;
        }
        if self.passphrase.read(cx).value().trim().is_empty() {
            self.error = Some("Passphrase required".into());
            cx.notify();
            return false;
        }
        true
    }

    fn render_banner(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let pinned = self.target_pinned;
        let mut target = div()
            .font_family(theme::fonts::MONO)
            .text_size(px(11.0))
            .px_1()
            .rounded(px(2.0))
            .text_color(theme::rgb(theme::colors::INK))
            .child("kernel_weights_fp16.bin");
        target = if pinned {
            target
                .bg(theme::hsla(theme::colors::PRIMARY, 0.12))
                .border_1()
                .border_color(theme::hsla(theme::colors::PRIMARY, 0.5))
        } else {
            target.bg(theme::rgb(theme::colors::SURFACE_HIGH))
        };
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
                    .gap_2()
                    .child(
                        svg()
                            .path("icons/shield-check.svg")
                            .w(px(14.0))
                            .h(px(14.0))
                            .flex_shrink_0()
                            .text_color(theme::rgb(theme::colors::PRIMARY)),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::ON_SURFACE))
                            .child("Enter session passphrase to unlock encrypted channel"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .ml(px(22.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::ON_SURFACE))
                            .child("payload and cryptographic sign-off matrix for"),
                    )
                    .child(
                        BaseButton::new(ElementId::Name("pp-channel".into()))
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::PRIMARY))
                            .on_click(move |_, _, cx: &mut App| {
                                view.update(cx, |v, cx| {
                                    v.target_pinned = !v.target_pinned;
                                    cx.notify();
                                });
                            })
                            .child("#image-processing"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .ml(px(22.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::ON_SURFACE))
                            .child("(Target:"),
                    )
                    .child(target)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::ON_SURFACE))
                            .child(")."),
                    ),
            )
    }

    fn render_entropy(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.hw_mode {
            return div()
                .text_size(px(11.0))
                .text_color(theme::rgb(theme::colors::MUTED))
                .child("Waiting for USB-C security key…")
                .into_any_element();
        }
        let score = entropy_score(&self.passphrase.read(cx).value());
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .font_family(theme::fonts::MONO)
                    .text_size(px(10.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child("ENTROPY:"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .children((0..4).map(|i| {
                        let mut seg = div().w(px(28.0)).h(px(6.0)).rounded(px(3.0));
                        seg = if i < score {
                            seg.bg(theme::rgb(theme::colors::SUCCESS))
                        } else {
                            seg.bg(theme::rgb(theme::colors::SURFACE_HIGH))
                        };
                        seg
                    })),
            )
            .into_any_element()
    }
}

impl Render for PassphraseDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let eye = cx.entity().clone();
        let cache_view = cx.entity().clone();
        let hw_view = cx.entity().clone();
        let auth_view = cx.entity().clone();

        let mut visibility = BaseButton::new(ElementId::Name("pp-visibility".into()))
            .w(px(24.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(2.0))
            .text_color(theme::rgb(theme::colors::MUTED));
        visibility = visibility.on_click(move |_, window, cx: &mut App| {
            eye.update(cx, |v, cx| {
                v.show_passphrase = !v.show_passphrase;
                v.passphrase.update(cx, |state, cx| {
                    state.toggle_masked(window, cx);
                });
                cx.notify();
            });
        });

        let (label, leading) = if self.hw_mode {
            ("HARDWARE TOKEN", "icons/lock-closed.svg")
        } else {
            ("ENTER PASSPHRASE", "icons/key.svg")
        };
        let field = FormField::new(label, self.passphrase.clone())
            .leading(leading)
            .font_family(theme::fonts::MONO)
            .trailing(
                visibility.child(
                    svg()
                        .path("icons/eye.svg")
                        .w(px(16.0))
                        .h(px(16.0))
                        .text_color(theme::rgb(theme::colors::MUTED)),
                ),
            );

        let hw_label = if self.hw_mode {
            "Use passphrase instead"
        } else {
            "Use hardware token instead"
        };

        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .p_5()
            .child(self.render_banner(cx))
            .child(field);
        if let Some(error) = self.error.clone() {
            body = body.child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::rgb(theme::colors::ERROR))
                    .child(error),
            );
        }
        body = body
            .child(self.render_entropy(cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .child(
                        DeckCheckbox::new("pp-cache", self.cache_key).on_toggle(
                            move |_, _, cx: &mut App| {
                                cache_view.update(cx, |v, cx| {
                                    v.cache_key = !v.cache_key;
                                    cx.notify();
                                });
                            },
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::rgb(theme::colors::INK))
                                    .child("Cache key in Secure Enclave for this session"),
                            )
                            .child(
                                div()
                                    .font_family(theme::fonts::MONO)
                                    .text_size(px(11.0))
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("(30 min auto-lock)"),
                            ),
                    ),
            )
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
                                    .path("icons/globe.svg")
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .flex_shrink_0()
                                    .text_color(theme::rgb(theme::colors::PRIMARY)),
                            )
                            .child(
                                BaseButton::new(ElementId::Name("pp-hw-toggle".into()))
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::rgb(theme::colors::PRIMARY))
                                    .on_click(move |_, _, cx: &mut App| {
                                        hw_view.update(cx, |v, cx| {
                                            v.hw_mode = !v.hw_mode;
                                            v.error = None;
                                            cx.notify();
                                        });
                                    })
                                    .child(hw_label),
                            ),
                    )
                    .child(
                        div()
                            .font_family(theme::fonts::MONO)
                            .text_size(px(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("USB-C READY"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .mt_2()
                    .child(
                        BaseButton::new(ElementId::Name("pp-cancel".into()))
                            .h(px(40.0))
                            .px_5()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::radius::DEFAULT))
                            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                            .border_1()
                            .border_color(theme::rgb(theme::colors::GRID_L2))
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::INK))
                            .on_click(|_, window, _| window.remove_window())
                            .child("Cancel"),
                    )
                    .child(
                        BaseButton::new(ElementId::Name("pp-auth".into()))
                            .h(px(40.0))
                            .px_5()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .rounded(px(theme::radius::DEFAULT))
                            .bg(theme::rgb(theme::colors::PRIMARY))
                            .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                            .text_size(px(13.0))
                            .font_weight(FontWeight::BOLD)
                            .on_click(move |_, window, cx: &mut App| {
                                if auth_view.update(cx, |v, cx| v.try_authenticate(cx)) {
                                    window.remove_window();
                                }
                            })
                            .child(
                                svg()
                                    .path("icons/lock-closed.svg")
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                            )
                            .child("AUTHENTICATE"),
                    ),
            );

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L2))
            .child(
                deferred(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .right(px(0.0))
                        .bottom(px(0.0))
                        .child(
                            div()
                                .absolute()
                                .top(px(10.0))
                                .left(px(10.0))
                                .child(CornerMark::new(Corner::TopLeft)),
                        )
                        .child(
                            div()
                                .absolute()
                                .top(px(10.0))
                                .right(px(10.0))
                                .child(CornerMark::new(Corner::TopRight)),
                        )
                        .child(
                            div()
                                .absolute()
                                .bottom(px(10.0))
                                .left(px(10.0))
                                .child(CornerMark::new(Corner::BottomLeft)),
                        )
                        .child(
                            div()
                                .absolute()
                                .bottom(px(10.0))
                                .right(px(10.0))
                                .child(CornerMark::new(Corner::BottomRight)),
                        ),
                )
                .with_priority(10),
            )
            .child(body)
    }
}
