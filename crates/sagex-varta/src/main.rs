pub mod app;
pub mod assets;
pub mod components;
pub mod layout;
pub mod routes;
pub mod seed;
pub mod theme;

use gpui_kit::base::actions::Cancel;
use gpui_kit::component::WindowExt;
use gpui_kit::*;

fn main() {
    /// Debug switch: boot the standalone sign-in window as the main
    /// screen. Flip to false to restore the chat window. The chat code
    /// path below is untouched.
    const DEBUG_LOGIN_MAIN: bool = false;
    /// Debug switch: boot the standalone sign-up window as the main
    /// screen instead of sign-in. Takes precedence when true.
    const DEBUG_SIGNUP_MAIN: bool = false;
    /// Debug switch: boot the standalone splash (initialization) window
    /// as the first screen. Takes precedence when true.
    const DEBUG_SPLASH_MAIN: bool = false;
    /// Debug switch: boot the standalone passphrase dialog as the first
    /// screen. Takes precedence when true.
    const DEBUG_PASSPHRASE_MAIN: bool = false;
    /// Debug switch: boot the profile page as the first screen. Takes
    /// precedence when true.
    const DEBUG_PROFILE_MAIN: bool = false;
    /// Debug switch: boot the licenses window as the first screen. Takes
    /// precedence when true.
    const DEBUG_LICENSE_MAIN: bool = false;

    gpui_kit::application()
        .with_assets(assets::VartaAssets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            // Sheets' built-in close buttons dispatch `Cancel`, which only
            // reaches this app-global handler. The dispatch still holds the
            // window borrowed, so `update` here always fails: defer the
            // close until effects flush. Guarded to a no-op with no sheet.
            cx.on_action(|_: &Cancel, cx: &mut App| {
                cx.defer(|cx| {
                    if let Some(window) = cx.active_window() {
                        let _ = window.update(cx, |_, window, cx| {
                            if window.has_active_sheet(cx) {
                                window.close_sheet(cx);
                            }
                        });
                    }
                });
            });
            if DEBUG_LICENSE_MAIN {
                let license_opts = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(
                            px(routes::licenses::LICENSE_WINDOW_SIZE[0]),
                            px(routes::licenses::LICENSE_WINDOW_SIZE[1]),
                        ),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Licenses".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                gpui_kit::open_window(license_opts, cx, |window, cx| {
                    cx.new(|cx| routes::licenses::LicensePage::new(window, cx))
                })
                .expect("failed to open LicenseWindow");
                return;
            }
            if DEBUG_PROFILE_MAIN {
                let profile_opts = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(
                            px(routes::profile::PROFILE_WINDOW_SIZE[0]),
                            px(routes::profile::PROFILE_WINDOW_SIZE[1]),
                        ),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Profile".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                gpui_kit::open_window(profile_opts, cx, |window, cx| {
                    cx.new(|cx| routes::profile::ProfilePage::new(window, cx))
                })
                .expect("failed to open ProfileWindow");
                return;
            }
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
