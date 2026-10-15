//! Sign-in page — manual `gpui-base` expansion only.
//!
//! Route page (see `DEBUG_LOGIN_MAIN` in `main.rs` for the standalone
//! debug window). Owns its email/password [`InputState`] entities plus
//! the visibility flag. Layout follows the blueprint terminal spec with
//! own code and fluid dimensions: title header, corner registration
//! marks, form panel beside a live-look schematic panel. Only the
//! visibility toggle acts locally; auth buttons are no-ops for now.

use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::input::InputState;
use gpui_kit::*;

use crate::components::form_field::FormField;
use crate::components::mono_link::MonoLink;
use crate::components::sso_button::SsoButton;
use crate::theme;

/// Corner registration mark: L-shaped right-angle bracket (16px bars)
/// for one frame corner, following the blueprint spec.
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

/// Locked content size of the sign-in page. Single source for the
/// `main.rs` bounds/min-size.
pub const LOGIN_WINDOW_SIZE: [f32; 2] = [660.0, 441.0];

pub struct LoginPage {
    email: Entity<InputState>,
    password: Entity<InputState>,
    show_password: bool,
    phase: LoginPhase,
}

/// Sign-in form versus key-reset flow. No backend: reset only
/// captures the account and confirms the link was sent.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginPhase {
    SignIn,
    ForgotKey,
    LinkSent,
}

impl LoginPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let email = cx.new(|cx| InputState::new(window, cx).placeholder("admin@sagex.com"));
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter security key")
                .masked(true)
        });
        Self {
            email,
            password,
            show_password: false,
            phase: LoginPhase::SignIn,
        }
    }

    pub fn open(cx: &mut App) -> bool {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(
                    px(LOGIN_WINDOW_SIZE[0]),
                    px(LOGIN_WINDOW_SIZE[1]),
                ),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Sign In".into()),
                ..Default::default()
            }),
            is_resizable: false,
            window_min_size: Some(size(
                px(LOGIN_WINDOW_SIZE[0]),
                px(LOGIN_WINDOW_SIZE[1]),
            )),
            ..Default::default()
        };
        gpui_kit::open_window(opts, cx, |window, cx| {
            cx.new(|cx| LoginPage::new(window, cx))
        })
        .is_ok()
    }

    fn toggle_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_password = !self.show_password;
        self.password.update(cx, |state, cx| {
            state.toggle_masked(window, cx);
        });
        cx.notify();
    }

    fn enter_forgot(&mut self, cx: &mut Context<Self>) {
        self.phase = LoginPhase::ForgotKey;
        cx.notify();
    }

    fn back_signin(&mut self, cx: &mut Context<Self>) {
        self.phase = LoginPhase::SignIn;
        cx.notify();
    }

    fn send_link(&mut self, cx: &mut Context<Self>) {
        self.phase = LoginPhase::LinkSent;
        cx.notify();
    }
}

impl Render for LoginPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let eye = cx.entity().clone();
        let view = cx.entity().clone();

        let mut visibility = BaseButton::new(ElementId::Name("login-visibility".into()))
            .w(px(24.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(2.0))
            .text_color(theme::rgb(theme::colors::MUTED));
        visibility = visibility.on_click(move |_, window, cx: &mut App| {
            eye.update(cx, |view, cx| view.toggle_password(window, cx));
        });

        let form_body: AnyElement = match self.phase {
            LoginPhase::SignIn => div()
                .flex()
                .flex_col()
                .gap_4()
                .w_full()
                .child(
                    FormField::new("EMAIL OR OPERATOR ID", self.email.clone())
                        .leading("icons/at-symbol.svg"),
                )
                .child(
                    FormField::new("PASSWORD / KEY", self.password.clone())
                        .leading("icons/lock-closed.svg")
                        .header_extra({
                            let view = view.clone();
                            MonoLink::new("FORGOT KEY?").on_click(
                                move |_, _, cx: &mut App| {
                                    view.update(cx, |view, cx| view.enter_forgot(cx));
                                },
                            )
                        })
                        .trailing(
                            visibility.child(
                                svg()
                                    .path("icons/eye.svg")
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .text_color(theme::rgb(theme::colors::MUTED)),
                            ),
                        ),
                )
                .child(
                    BaseButton::new(ElementId::Name("login-submit".into()))
                        .w_full()
                        .h(px(44.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .rounded(px(theme::radius::DEFAULT))
                        .bg(theme::rgb(theme::colors::PRIMARY))
                        .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                        .text_size(px(15.0))
                        .font_weight(FontWeight::BOLD)
                        .child(
                            svg()
                                .path("icons/key.svg")
                                .w(px(15.0))
                                .h(px(15.0))
                                .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                        )
                        .child("Log In"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .w_full()
                        .child(
                            div()
                                .flex_1()
                                .h(px(1.0))
                                .bg(theme::rgb(theme::colors::SURFACE_HIGH)),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::rgb(theme::colors::MUTED))
                                .child("OR SIGN IN WITH"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .h(px(1.0))
                                .bg(theme::rgb(theme::colors::SURFACE_HIGH)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .w_full()
                        .child(SsoButton::new(
                            "login-passkey",
                            "icons/fingerprint.svg",
                            theme::colors::PRIMARY,
                            "Passkey ",
                        ))
                        .child(SsoButton::new(
                            "login-sso",
                            "icons/globe.svg",
                            theme::colors::TERTIARY_CONTAINER,
                            "SSO Gateway",
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .w_full()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::rgb(theme::colors::MUTED))
                                .child("Don't have an account?"),
                        )
                        .child({
                            MonoLink::new("Register now").on_click(
                                |_, window, cx: &mut App| {
                                    if crate::routes::signup::SignupPage::open(cx) {
                                        window.remove_window();
                                    }
                                },
                            )
                        }),
                )
                .into_any_element(),
            LoginPhase::ForgotKey => div()
                .flex()
                .flex_col()
                .gap_4()
                .w_full()
                .child(
                    FormField::new("ACCOUNT EMAIL", self.email.clone())
                        .leading("icons/mail.svg"),
                )
                .child({
                    let view = view.clone();
                    BaseButton::new(ElementId::Name("login-send-link".into()))
                        .w_full()
                        .h(px(44.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .rounded(px(theme::radius::DEFAULT))
                        .bg(theme::rgb(theme::colors::PRIMARY))
                        .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                        .text_size(px(15.0))
                        .font_weight(FontWeight::BOLD)
                        .on_click(move |_, _, cx: &mut App| {
                            view.update(cx, |view, cx| view.send_link(cx));
                        })
                        .child(
                            svg()
                                .path("icons/mail.svg")
                                .w(px(15.0))
                                .h(px(15.0))
                                .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                        )
                        .child("SEND RESET LINK")
                })
                .child({
                    let view = view.clone();
                    MonoLink::new("Back to sign in").on_click(move |_, _, cx: &mut App| {
                        view.update(cx, |view, cx| view.back_signin(cx));
                    })
                })
                .into_any_element(),
            LoginPhase::LinkSent => div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .w_full()
                .py_4()
                .child(
                    svg()
                        .path("icons/check.svg")
                        .w(px(24.0))
                        .h(px(24.0))
                        .text_color(theme::rgb(theme::colors::SUCCESS)),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::rgb(theme::colors::INK))
                        .child("RESET LINK SENT"),
                )
                .child(
                    div()
                        .w_full()
                        .text_size(px(11.0))
                        .text_color(theme::rgb(theme::colors::MUTED))
                        .text_align(TextAlign::Center)
                        .child("Check your inbox for the reset link."),
                )
                .child({
                    let view = view.clone();
                    MonoLink::new("Back to sign in").on_click(move |_, _, cx: &mut App| {
                        view.update(cx, |view, cx| view.back_signin(cx));
                    })
                })
                .into_any_element(),
        };

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .overflow_hidden()
            .child(
                // Topmost paint layer (the Kit's z-index equivalent):
                // the four registration angles live first in the tree
                // but paint above everything.
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
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .w_full()
                    .px_6()
                    .py_4()
                    .bg(theme::rgb(theme::colors::SURFACE_LOW))
                    .border_b_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .w(px(40.0))
                                    .h(px(40.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme::hsla(theme::colors::PRIMARY, 0.1))
                                    .rounded(px(theme::radius::DEFAULT))
                                    .child(
                                        svg()
                                            .path("icons/shield-check.svg")
                                            .w(px(20.0))
                                            .h(px(20.0))
                                            .text_color(theme::rgb(theme::colors::PRIMARY)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(18.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::rgb(theme::colors::ON_SURFACE))
                                            .child("Log in "),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme::rgb(theme::colors::MUTED))
                                            .child("Secure Attribution And Governance Engine"),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .flex_grow(7.0)
                            .min_w_0()
                            .p_6()
                            .child(form_body),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .flex_grow(5.0)
                            .min_w_0()
                            .p_6()
                            .bg(theme::rgb(theme::colors::SURFACE_LOW))
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .w_full()
                                    .child(
                                        // Full-color art: the Kit's `svg()`
                                        // paints a monochrome mask, while
                                        // `img()` rasterizes SVGs as authored.
                                        img("images/login-scene.svg").w(px(224.0)).h(px(224.0)),
                                    ),
                            ),
                    ),
            )
    }
}
