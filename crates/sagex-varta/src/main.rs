pub mod app;
pub mod assets;
pub mod components;
pub mod layout;
pub mod routes;
pub mod seed;
pub mod theme;

use gpui_kit::*;

fn main() {
    /// Debug switch: boot the standalone sign-in window as the main
    /// screen. Flip to false to restore the chat window. The chat code
    /// path below is untouched.
    const DEBUG_LOGIN_MAIN: bool = true;
    /// Debug switch: boot the standalone sign-up window as the main
    /// screen instead of sign-in. Takes precedence when true.
    const DEBUG_SIGNUP_MAIN: bool = true;
    /// Debug switch: boot the standalone splash (initialization) window
    /// as the first screen. Takes precedence when true.
    const DEBUG_SPLASH_MAIN: bool = true;
    /// Debug switch: boot the standalone passphrase dialog as the first
    /// screen. Takes precedence when true.
    const DEBUG_PASSPHRASE_MAIN: bool = true;

    gpui_kit::application()
        .with_assets(assets::VartaAssets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            if DEBUG_PASSPHRASE_MAIN {
                if routes::passphrase::PassphraseDialog::open(cx) {
                    return;
                }
            }
            if DEBUG_SPLASH_MAIN {
                if routes::splash::SplashPage::open(cx) {
                    return;
                }
            }
            if DEBUG_SIGNUP_MAIN {
                let signup_opts = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(
                            px(routes::signup::SIGNUP_WINDOW_SIZE[0]),
                            px(routes::signup::SIGNUP_WINDOW_SIZE[1]),
                        ),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Sign Up".into()),
                        ..Default::default()
                    }),
                    is_resizable: false,
                    window_min_size: Some(size(
                        px(routes::signup::SIGNUP_WINDOW_SIZE[0]),
                        px(routes::signup::SIGNUP_WINDOW_SIZE[1]),
                    )),
                    ..Default::default()
                };
                gpui_kit::open_window(signup_opts, cx, |window, cx| {
                    cx.new(|cx| routes::signup::SignupPage::new(window, cx))
                })
                .expect("failed to open SignupWindow");
                return;
            }
            if DEBUG_LOGIN_MAIN {
                let login_opts = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(
                            px(routes::login::LOGIN_WINDOW_SIZE[0]),
                            px(routes::login::LOGIN_WINDOW_SIZE[1]),
                        ),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Sign In".into()),
                        ..Default::default()
                    }),
                    is_resizable: false,
                    window_min_size: Some(size(
                        px(routes::login::LOGIN_WINDOW_SIZE[0]),
                        px(routes::login::LOGIN_WINDOW_SIZE[1]),
                    )),
                    ..Default::default()
                };
                gpui_kit::open_window(login_opts, cx, |window, cx| {
                    cx.new(|cx| routes::login::LoginPage::new(window, cx))
                })
                .expect("failed to open LoginWindow");
                return;
            }
            // let opts = WindowOptions {
            //     window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            //         None,
            //         size(px(1280.0), px(800.0)),
            //         cx,
            //     ))),
            //     titlebar: Some(TitlebarOptions {
            //         title: Some("SageX Varta".into()),
            //         ..Default::default()
            //     }),
            //     ..Default::default()
            // };
            // gpui_kit::open_window(opts, cx, |window, cx| {
            //     cx.new(|cx| app::AppView::new(window, cx))
            // })
            // .expect("failed to open window");
            let chat_opts = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1000.0), px(700.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("ChatWindow".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            gpui_kit::open_window(chat_opts, cx, |window, cx| {
                cx.new(|cx| layout::chat_window::ChatWindow::new(window, cx))
            })
            .expect("failed to open ChatWindow");
        });
}
