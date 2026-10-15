use crate::components::blueprint_button::BlueprintButton;
use crate::routes::{HelloPage, LoginPage, OtpPage, SetupPage, SignupPage};
use crate::theme;
use gpui_kit::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Hello,
    Login,
    Signup,
    Otp,
    Setup,
}

impl Route {
    pub fn label(&self) -> &'static str {
        match self {
            Route::Hello => "Hello",
            Route::Login => "Login",
            Route::Signup => "Signup",
            Route::Otp => "Otp",
            Route::Setup => "Setup",
        }
    }

    pub fn all() -> &'static [Route] {
        &[
            Route::Hello,
            Route::Login,
            Route::Signup,
            Route::Otp,
            Route::Setup,
        ]
    }
}

pub struct AppView {
    route: Route,
    hello: Entity<HelloPage>,
    login: Entity<LoginPage>,
    signup: Entity<SignupPage>,
    otp: Entity<OtpPage>,
    setup: Entity<SetupPage>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            route: Route::Hello,
            hello: cx.new(|_| HelloPage {}),
            login: cx.new(|cx| LoginPage::new(window, cx)),
            signup: cx.new(|cx| SignupPage::new(window, cx)),
            otp: cx.new(|_| OtpPage {}),
            setup: cx.new(|_| SetupPage {}),
        }
    }

    fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.route = route;
        cx.notify();
    }

    fn render_nav(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .bg(theme::rgb(theme::colors::SURFACE_LOW))
            .children(Route::all().iter().map(|r| {
                let route = *r;
                let active = self.route == route;
                let mut btn =
                    BlueprintButton::new(SharedString::from(format!("nav-{}", route.label())))
                        .label(route.label());
                if active {
                    btn = btn.primary();
                } else {
                    btn = btn.ghost();
                }
                btn.on_click(cx.listener(move |view, _, _, cx| {
                    view.set_route(route, cx);
                }))
            }))
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match self.route {
            Route::Hello => self.hello.clone().into_any_element(),
            Route::Login => self.login.clone().into_any_element(),
            Route::Signup => self.signup.clone().into_any_element(),
            Route::Otp => self.otp.clone().into_any_element(),
            Route::Setup => self.setup.clone().into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::rgb(theme::colors::BACKGROUND))
            .child(self.render_nav(cx))
            .child(page)
    }
}
