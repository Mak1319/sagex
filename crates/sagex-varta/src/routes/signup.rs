//! Sign-up page — manual `gpui-base` expansion only.
//!
//! Route page and temporary debug main (see `DEBUG_SIGNUP_MAIN` in
//! `main.rs`). Clone of the sign-in page structure with the registration
//! fields: full name, email, username, password, confirm password and a
//! terms checkbox. All state is local; buttons are no-ops for now.

use std::time::{Duration, Instant};

use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::input::{Input, InputState, InputEvent};
use gpui_kit::base::motion::{Presence, Transition};
use gpui_kit::*;

use crate::components::deck_checkbox::DeckCheckbox;
use crate::components::form_field::FormField;
use crate::components::mono_link::MonoLink;
use crate::components::otp_boxes::OtpBoxes;
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

/// Locked content size of the sign-up page: same base as sign-in.
/// Single source for the `main.rs` bounds/min-size.
pub const SIGNUP_WINDOW_SIZE: [f32; 2] = [660.0, 441.0];

/// Number of wizard steps: name, email, username, password, confirm,
/// terms. The last step carries the checkbox instead of a field.
/// Verification runs as a separate phase after the wizard.
const STEP_COUNT: usize = 6;

/// Seconds a verification code stays valid.
const OTP_WINDOW_SECS: u64 = 120;

/// Wizard form versus code verification: separate phases, separate
/// layouts, same frame.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Form,
    Verify,
}

pub struct SignupPage {
    full_name: Entity<InputState>,
    email: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    confirm: Entity<InputState>,
    otp: Entity<InputState>,
    agreed: bool,
    show_password: bool,
    show_confirm: bool,
    step: usize,
    forward: bool,
    phase: Phase,
    otp_since: Option<Instant>,
    /// Failed-advance flag: shows the `Required.` message on empty
    /// fields. Cleared on every step change.
    tried: bool,
    /// Step awaiting focus. Consumed in render (the one place holding
    /// both the fresh layout and a window handle).
    focus_step: Option<usize>,
    /// One observer per input: any keystroke re-renders so buttons
    /// reflect live validity. Only held, never read.
    _watches: Vec<Subscription>,
}

impl SignupPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let full_name = cx.new(|cx| InputState::new(window, cx).placeholder("Ada Lovelace"));
        let email = cx.new(|cx| InputState::new(window, cx).placeholder("ada@apex-cad.io"));
        let username = cx.new(|cx| InputState::new(window, cx).placeholder("ada-lovelace"));
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Choose security key")
                .masked(true)
        });
        let confirm = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Repeat security key")
                .masked(true)
        });
        let otp = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("––––––")
                .mask_pattern("999999")
        });
        let mut _watches: Vec<Subscription> = [
            &full_name,
            &email,
            &username,
            &password,
            &confirm,
            &otp,
        ]
        .iter()
        .map(|field| cx.observe(*field, |_, _, cx| cx.notify()))
        .collect();
        for field in [&full_name, &email, &username, &password, &confirm] {
            _watches.push(cx.subscribe(
                field,
                |view: &mut SignupPage,
                 _: Entity<InputState>,
                 event: &InputEvent,
                 cx: &mut Context<SignupPage>| {
                    if matches!(event, InputEvent::PressEnter { .. })
                        && view.phase == Phase::Form
                    {
                        view.try_advance(cx);
                    }
                },
            ));
        }
        Self {
            full_name,
            email,
            username,
            password,
            confirm,
            otp,
            agreed: false,
            show_password: false,
            show_confirm: false,
            step: 0,
            forward: true,
            phase: Phase::Form,
            otp_since: None,
            tried: false,
            focus_step: Some(0),
            _watches,
        }
    }

    fn field(&self, step: usize) -> Entity<InputState> {
        match step {
            0 => self.full_name.clone(),
            1 => self.email.clone(),
            2 => self.username.clone(),
            3 => self.password.clone(),
            _ => self.confirm.clone(),
        }
    }

    fn step_value(&self, step: usize, cx: &App) -> String {
        self.field(step).read(cx).value().to_string()
    }

    fn step_valid(&self, step: usize, cx: &App) -> bool {
        match step {
            0 => valid_name(&self.step_value(step, cx)),
            1 => valid_email(&self.step_value(step, cx)),
            2 => valid_username(&self.step_value(step, cx)),
            3 => valid_password(&self.step_value(step, cx)),
            4 => {
                let confirm = self.step_value(step, cx);
                valid_password(&confirm)
                    && confirm == self.password.read(cx).value().to_string()
            }
            _ => true,
        }
    }

    pub fn open(cx: &mut App) -> bool {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(
                    px(SIGNUP_WINDOW_SIZE[0]),
                    px(SIGNUP_WINDOW_SIZE[1]),
                ),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Sign Up".into()),
                ..Default::default()
            }),
            is_resizable: false,
            window_min_size: Some(size(
                px(SIGNUP_WINDOW_SIZE[0]),
                px(SIGNUP_WINDOW_SIZE[1]),
            )),
            ..Default::default()
        };
        gpui_kit::open_window(crate::assets::with_app_id(opts), cx, |window, cx| {
            cx.new(|cx| SignupPage::new(window, cx))
        })
        .is_ok()
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        if self.step + 1 < STEP_COUNT {
            self.step += 1;
            self.forward = true;
            self.focus_step = Some(self.step);
            self.tried = false;
            cx.notify();
        }
    }

    fn retreat(&mut self, cx: &mut Context<Self>) {
        if self.step > 0 {
            self.step -= 1;
            self.forward = false;
            self.focus_step = Some(self.step);
            self.tried = false;
            cx.notify();
        }
    }

    fn try_advance(&mut self, cx: &mut Context<Self>) {
        let step = self.step.min(STEP_COUNT - 1);
        if self.step_valid(step, cx) {
            self.advance(cx);
        } else {
            self.tried = true;
            cx.notify();
        }
    }

    /// Short message describing what is wrong with the current step, if
    /// anything, plus its tone. Empty fields stay quiet until an advance
    /// is attempted; password length is a warning, the rest are errors.
    fn step_issue(&self, step: usize, cx: &App) -> Option<(String, u32)> {
        let error = theme::colors::ERROR;
        let value = self.step_value(step, cx);
        if value.trim().is_empty() {
            return if self.tried {
                Some(("Required.".to_string(), error))
            } else {
                None
            };
        }
        match step {
            0 => (!valid_name(&value))
                .then(|| ("Enter your full name.".to_string(), error)),
            1 => (!valid_email(&value))
                .then(|| ("Enter a valid email address.".to_string(), error)),
            2 => (!valid_username(&value)).then(|| {
                (
                    "3+ letters, numbers, _ - .".to_string(),
                    error,
                )
            }),
            3 => (!valid_password(&value)).then(|| {
                (
                    "Use at least 8 characters.".to_string(),
                    theme::colors::WARNING,
                )
            }),
            4 => {
                let confirm = value;
                if !valid_password(&confirm) {
                    Some((
                        "Use at least 8 characters.".to_string(),
                        theme::colors::WARNING,
                    ))
                } else if confirm != self.password.read(cx).value().to_string() {
                    Some(("Keys do not match.".to_string(), error))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn toggle_agreed(&mut self, cx: &mut Context<Self>) {
        self.agreed = !self.agreed;
        cx.notify();
    }

    fn toggle_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_password = !self.show_password;
        self.password.update(cx, |state, cx| {
            state.toggle_masked(window, cx);
        });
        cx.notify();
    }

    fn toggle_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_confirm = !self.show_confirm;
        self.confirm.update(cx, |state, cx| {
            state.toggle_masked(window, cx);
        });
        cx.notify();
    }

    fn focus_otp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.otp.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    fn enter_verify(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Verify;
        self.forward = true;
        self.otp_since = Some(Instant::now());
        cx.notify();
    }

    fn back_to_form(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Form;
        self.forward = false;
        cx.notify();
    }

    fn resend_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.otp.update(cx, |state, cx| {
            state.replace_all("", window, cx);
        });
        self.otp_since = Some(Instant::now());
        self.focus_otp(window, cx);
        cx.notify();
    }

    fn otp_remaining(&self) -> u64 {
        self.otp_since
            .map(|since| OTP_WINDOW_SECS.saturating_sub(since.elapsed().as_secs()))
            .unwrap_or(OTP_WINDOW_SECS)
    }
}

fn mask_address(addr: &str) -> String {
    match addr.split_once('@') {
        Some((local, domain)) => format!(
            "{}****@{}",
            local.chars().next().unwrap_or('*'),
            domain
        ),
        None => addr.to_string(),
    }
}

fn valid_name(value: &str) -> bool {
    value.trim().chars().count() >= 2
}

fn valid_email(value: &str) -> bool {
    let value = value.trim();
    match value.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        }
        None => false,
    }
}

fn valid_username(value: &str) -> bool {
    let value = value.trim();
    value.len() >= 3
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn valid_password(value: &str) -> bool {
    value.len() >= 8
}

fn visibility_button(
    id: &str,
    view: Entity<SignupPage>,
    toggle: fn(&mut SignupPage, &mut Window, &mut Context<SignupPage>),
) -> impl IntoElement {
    let mut button = BaseButton::new(ElementId::Name(SharedString::from(id.to_string())))
        .w(px(24.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(2.0))
        .text_color(theme::rgb(theme::colors::MUTED));
    button = button.on_click(move |_, window, cx: &mut App| {
        view.update(cx, |view, cx| toggle(view, window, cx));
    });
    button.child(
        svg()
            .path("icons/eye.svg")
            .w(px(16.0))
            .h(px(16.0))
            .text_color(theme::rgb(theme::colors::MUTED)),
    )
}

impl Render for SignupPage {
    #[rustfmt::skip]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let step = self.step.min(STEP_COUNT - 1);
        let last = step + 1 == STEP_COUNT;

        // Motion enter slide: a fresh key per step replays 0 → 1,
        // mapped to a directional offset plus fade.
        let sample = Presence::new(format!("signup-step-{step}"), true)
            .transition(Transition::new(Duration::from_millis(180)))
            .sample(window, cx);
        let dir = if self.forward { 1.0 } else { -1.0 };
        let offset = dir * 40.0 * (1.0 - sample.progress);

        // Focus follows the step: render is the one place holding both
        // the fresh layout and a window handle.
        if self.focus_step == Some(step) {
            self.focus_step = None;
            self.field(step).update(cx, |state, cx| {
                state.focus(window, cx);
            });
        }

        let field = match step {
            0 => FormField::new("FULL NAME", self.full_name.clone()).leading("icons/user-plus.svg"),
            1 => FormField::new("EMAIL", self.email.clone()).leading("icons/mail.svg"),
            2 => FormField::new("USERNAME", self.username.clone()).leading("icons/at-symbol.svg"),
            3 => FormField::new("PASSWORD", self.password.clone())
                .leading("icons/lock-closed.svg")
                .trailing(visibility_button(
                    "signup-visibility",
                    view.clone(),
                    SignupPage::toggle_password,
                )),
            _ => FormField::new("CONFIRM PASSWORD", self.confirm.clone())
                .leading("icons/lock-closed.svg")
                .trailing(visibility_button(
                    "signup-confirm",
                    view.clone(),
                    SignupPage::toggle_confirm,
                )),
        };

        let mut agree = DeckCheckbox::new("signup-agree", self.agreed);
        {
            let view = view.clone();
            agree = agree.on_toggle(move |_, _, cx: &mut App| {
                view.update(cx, |view, cx| view.toggle_agreed(cx));
            });
        }

        let progress = div()
            .text_size(px(10.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::rgb(theme::colors::MUTED))
            .child(SharedString::from(format!(
                "STEP {} OF {}",
                step + 1,
                STEP_COUNT
            )));

        let mut slide = div()
            .relative()
            .w_full()
            .gap_4()
            .left(px(offset))
            .opacity(sample.progress);
        if last {
            slide = slide.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(agree)
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("I ACCEPT THE TERMS AND CONDITIONS"),
                    ),
            );
        } else {
            slide = slide.child(field);
        }
        if self.phase == Phase::Form {
            if let Some((message, tone)) = self.step_issue(step, cx) {
                slide = slide.child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::rgb(tone))
                        .child(SharedString::from(message)),
                );
            }
        }

        // Live validity: the action button below renders disabled
        // until the current step validates (plus terms on last).
        let valid = self.step_valid(step, cx);
        let actionable = if last { valid && self.agreed } else { valid };

        let mut back_btn = BaseButton::new(ElementId::Name("signup-back".into()))
            .w(px(36.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::radius::DEFAULT))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L2))
            .text_color(theme::rgb(theme::colors::MUTED));
        {
            let view = view.clone();
            back_btn = back_btn
                .on_click(move |_, _, cx: &mut App| {
                    view.update(cx, |view, cx| view.retreat(cx));
                })
                .child(
                    svg()
                        .path("icons/arrow-left.svg")
                        .w(px(16.0))
                        .h(px(16.0))
                        .text_color(theme::rgb(theme::colors::MUTED)),
                );
        }

        let mut action_btn = BaseButton::new(ElementId::Name("signup-action".into()))
            .w(px(36.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::radius::DEFAULT));
        if actionable {
            action_btn = action_btn
                .bg(theme::rgb(theme::colors::PRIMARY))
                .border_1()
                .border_color(theme::rgb(theme::colors::PRIMARY))
                .text_color(theme::rgb(theme::colors::ON_PRIMARY));
            if last {
                let view = view.clone();
                action_btn = action_btn
                    .on_click(move |_, window, cx: &mut App| {
                        view.update(cx, |view, cx| {
                            if view.agreed {
                                view.enter_verify(cx);
                                view.focus_otp(window, cx);
                            }
                        });
                    })
                    .child(
                    svg()
                        .path("icons/arrow-right.svg")
                        .w(px(16.0))
                        .h(px(16.0))
                        .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                );
            } else {
                let view = view.clone();
                action_btn = action_btn
                    .on_click(move |_, _, cx: &mut App| {
                        view.update(cx, |view, cx| view.try_advance(cx));
                    })
                    .child(
                        svg()
                            .path("icons/arrow-right.svg")
                            .w(px(16.0))
                            .h(px(16.0))
                            .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                    );
            }
        } else {
            action_btn = action_btn
                .bg(theme::rgb(theme::colors::INSET))
                .border_1()
                .border_color(theme::rgb(theme::colors::GRID_L2))
                .text_color(theme::rgb(theme::colors::MUTED))
                .child(
                    svg()
                        .path("icons/arrow-right.svg")
                        .w(px(16.0))
                        .h(px(16.0))
                        .text_color(theme::rgb(theme::colors::MUTED)),
                );
        }

        let mut nav = div().flex().flex_row().items_center().gap_2().w_full();
        if step > 0 {
            nav = nav.child(back_btn);
        }
        nav = nav.child(div().flex_1()).child(action_btn);

        let prompt = div()
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
                    .child("Already have an account?"),
            )
            .child(MonoLink::new("Sign in").on_click(|_, window, cx: &mut App| {
                if crate::routes::login::LoginPage::open(cx) {
                    window.remove_window();
                }
            }));

        // --- Verify phase pieces (OTP after terms) ---
        let code = self.otp.read(cx).value().to_string();
        let remaining = self.otp_remaining();
        if self.phase == Phase::Verify && remaining > 0 {
            window.request_animation_frame();
        }
        let tick = format!("{:02}:{:02} SECONDS", remaining / 60, remaining % 60);
        let masked = mask_address(&self.email.read(cx).value());
        let elapsed = OTP_WINDOW_SECS.saturating_sub(remaining) as f32;
        let left = OTP_WINDOW_SECS as f32 - elapsed;

        let verify_head = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .w_full()
            .child({
                let view = view.clone();
                BaseButton::new(ElementId::Name("otp-back".into()))
                    .w(px(28.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::radius::DEFAULT))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L2))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .on_click(move |_, _, cx: &mut App| {
                        view.update(cx, |view, cx| view.back_to_form(cx));
                    })
                    .child(
                        svg()
                            .path("icons/arrow-left.svg")
                            .w(px(14.0))
                            .h(px(14.0))
                            .text_color(theme::rgb(theme::colors::MUTED)),
                    )
            })
            .child(
                div()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child("VERIFY YOUR EMAIL"),
            );

        let banner = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .w_full()
            .px_3()
            .py_2()
            .bg(theme::rgb(theme::colors::SURFACE_CONTAINER))
            .rounded(px(theme::radius::DEFAULT))
            .child(
                div()
                    .w(px(32.0))
                    .h(px(32.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .rounded(px(2.0))
                    .child(
                        svg()
                            .path("icons/mail.svg")
                            .w(px(15.0))
                            .h(px(15.0))
                            .text_color(theme::rgb(theme::colors::PRIMARY)),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::rgb(theme::colors::INK))
                    .child("Code sent to"),
            )
            .child(
                div()
                    .px_2()
                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                    .rounded(px(2.0))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::rgb(theme::colors::PRIMARY))
                    .child(SharedString::from(masked)),
            );

        let boxes = {
            let view = view.clone();
            div()
                .id(ElementId::Name("otp-focus".into()))
                .flex()
                .flex_row()
                .justify_center()
                .w_full()
                .on_click(move |_, window, cx: &mut App| {
                    view.update(cx, |view, cx| view.focus_otp(window, cx));
                })
                .child(OtpBoxes::new(code.clone()))
        };
        let hidden_engine = div()
            .w(px(1.0))
            .h(px(1.0))
            .overflow_hidden()
            .opacity(0.0)
            .child(Input::new(&self.otp));

        let expiry = div()
            .flex()
            .flex_col()
            .gap_1()
            .w_full()
            .px_3()
            .py_2()
            .bg(theme::rgb(theme::colors::SURFACE_CONTAINER))
            .rounded(px(theme::radius::DEFAULT))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .child(
                        svg()
                            .path("icons/timer.svg")
                            .w(px(12.0))
                            .h(px(12.0))
                            .text_color(theme::rgb(theme::colors::PRIMARY)),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("Code expires in:"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::rgb(theme::colors::PRIMARY))
                            .child(SharedString::from(tick)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(4.0))
                    .flex()
                    .flex_row()
                    .rounded_full()
                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .overflow_hidden()
                    .child(div().flex_grow(elapsed).h_full().bg(theme::rgb(
                        theme::colors::PRIMARY,
                    )))
                    .child(div().flex_grow(left)),
            );

        let complete = code.len() == 6;
        let mut verify_btn =
            BaseButton::new(ElementId::Name("otp-verify".into()))
                .w_full()
                .h(px(44.0))
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap_2()
                .rounded(px(theme::radius::DEFAULT))
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD);
        if complete {
            verify_btn = verify_btn
                .bg(theme::rgb(theme::colors::PRIMARY))
                .text_color(theme::rgb(theme::colors::ON_PRIMARY))
                .on_click(|_, _, _: &mut App| {})
                .child(
                    svg()
                        .path("icons/check.svg")
                        .w(px(15.0))
                        .h(px(15.0))
                        .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                )
                .child("VERIFY CODE");
        } else {
            verify_btn = verify_btn
                .bg(theme::rgb(theme::colors::INSET))
                .border_1()
                .border_color(theme::rgb(theme::colors::GRID_L2))
                .text_color(theme::rgb(theme::colors::MUTED))
                .child(
                    svg()
                        .path("icons/check.svg")
                        .w(px(15.0))
                        .h(px(15.0))
                        .text_color(theme::rgb(theme::colors::MUTED)),
                )
                .child("VERIFY CODE");
        }

        let resend_row = {
            let view = view.clone();
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_2()
                .w_full()
                .child(
                    BaseButton::new(ElementId::Name("otp-resend".into()))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .py_1()
                        .px_2()
                        .rounded(px(2.0))
                        .bg(theme::rgb(theme::colors::SURFACE_CONTAINER))
                        .text_size(px(10.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::rgb(theme::colors::MUTED))
                        .on_click(move |_, window, cx: &mut App| {
                            view.update(cx, |view, cx| view.resend_code(window, cx));
                        })
                        .child(
                            svg()
                                .path("icons/refresh.svg")
                                .w(px(12.0))
                                .h(px(12.0))
                                .text_color(theme::rgb(theme::colors::MUTED)),
                        )
                        .child("Resend Code"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(
                            svg()
                                .path("icons/key.svg")
                                .w(px(12.0))
                                .h(px(12.0))
                                .text_color(theme::rgb(theme::colors::PRIMARY)),
                        )
                        .child(MonoLink::new("Use recovery code instead")),
                )
        };

        let divider = div()
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
                    .child("OR SIGN UP WITH"),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(1.0))
                    .bg(theme::rgb(theme::colors::SURFACE_HIGH)),
            );
        let sso_row = div()
            .flex()
            .flex_row()
            .gap_2()
            .w_full()
            .child(SsoButton::new(
                "signup-passkey",
                "icons/fingerprint.svg",
                theme::colors::PRIMARY,
                "Passkey ",
            ))
            .child(SsoButton::new(
                "signup-sso",
                "icons/globe.svg",
                theme::colors::TERTIARY_CONTAINER,
                "SSO Gateway",
            ));

        let (form_top, form_bottom): (AnyElement, AnyElement) = match self.phase {
            Phase::Form => {
                let mut bottom = div().flex().flex_col().gap_4().w_full();
                if step == 0 {
                    bottom = bottom.child(divider).child(sso_row);
                }
                (
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .w_full()
                        .child(progress)
                        .child(slide)
                        .into_any_element(),
                    bottom.child(nav).child(prompt).into_any_element(),
                )
            }
            Phase::Verify => (
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .child(verify_head)
                    .child(banner)
                    .child(boxes)
                    .child(hidden_engine)
                    .child(expiry)
                    .into_any_element(),
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_full()
                    .child(verify_btn)
                    .child(resend_row)
                    .into_any_element(),
            ),
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
                // This is the portion for the angles
                deferred(
                    div()
                        // .bg(rgb(0xff0000))
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
                    .size_full()
                    .flex()
                    .flex_col() 
                    .child(
                        // Header 
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .w_full()
                            // .size_full()
                            .px_6()
                            .py_4()
                            .bg(theme::rgb(theme::colors::SURFACE_LOW))
                            // .bg(theme::rgb(theme::colors::PRIMARY))
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
                                                    .text_color(theme::rgb(
                                                        theme::colors::ON_SURFACE,
                                                    ))
                                                    .child("Sign up "),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(theme::rgb(theme::colors::MUTED))
                                                    .child(
                                                        "Secure Attribution And Governance Engine",
                                                    ),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()

                            // bottom section 
                            .flex()
                            .flex_grow_1()
                            // .h_full()
                            .flex_row()
                            .w_full()
                            // .border_b_16()
                            // .border_color(rgb(0xffff00))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .flex_grow(7.0)
                                    .min_w_0()
                                    .p_6()
                                    .child(form_top)
                                    .child(form_bottom),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .flex_grow(5.0)
                                    .min_w_0()
                                    .h_full()
                                    .p_6()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme::rgb(theme::colors::SURFACE_LOW))
                                    .border_color(theme::rgb(theme::colors::GRID_L1))
                                    .border_l_1()
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
                                                img("images/login-scene.svg")
                                                    .w(px(224.0))
                                                    .h(px(224.0)),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
