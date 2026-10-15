//! Sagex Varta initialization screen — static UI only.
//!
//! Standalone route window (booted by `DEBUG_SPLASH_MAIN` in `main.rs`)
//! mirroring the reference splash composition: the "Sagex Varta"
//! wordmark, edition + version lines in Google Sans, "Powered by Nivara"
//! credit, and the scenery filling the right panel edge to edge. Art is pre-rasterized to PNG at build time (see
//! `build.rs`); no timers, no background work, no handoff — presentation
//! only.

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

/// Locked content size of the splash screen: 600x374 (500x312 ratio
/// scaled to 600 wide). Single source for the `main.rs` bounds/min-size.
pub const SPLASH_WINDOW_SIZE: [f32; 2] = [600.0, 374.0];

/// Left branding column width: one third of the window width.
const LEFT_WIDTH: f32 = SPLASH_WINDOW_SIZE[0] / 3.0;

/// Which top corner the hover close button lives in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CloseSide {
    Left,
    Right,
}

impl CloseSide {
    /// Platform default honoring the user's desktop layout: macOS traffic
    /// lights sit left, Windows right; on Linux the GNOME button-layout
    /// setting decides (`close` before the `:` = left). Falls back to
    /// right. `SAGEX_CLOSE_SIDE=left|right` always wins when set.
    fn detect() -> Self {
        if let Ok(side) = std::env::var("SAGEX_CLOSE_SIDE") {
            if side.eq_ignore_ascii_case("left") {
                return Self::Left;
            }
            if side.eq_ignore_ascii_case("right") {
                return Self::Right;
            }
        }
        if cfg!(target_os = "macos") {
            return Self::Left;
        }
        if cfg!(target_os = "windows") {
            return Self::Right;
        }
        Self::linux_layout().unwrap_or(Self::Right)
    }

    #[cfg(target_os = "linux")]
    fn linux_layout() -> Option<Self> {
        let out = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let layout = String::from_utf8_lossy(&out.stdout);
        let (left, _right) = layout.split_once(':')?;
        Some(if left.contains("close") {
            Self::Left
        } else {
            Self::Right
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn linux_layout() -> Option<Self> {
        None
    }
}

pub struct SplashPage {
    close_side: CloseSide,
}

impl SplashPage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            close_side: CloseSide::detect(),
        }
    }

    pub fn open(cx: &mut App) -> bool {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(SPLASH_WINDOW_SIZE[0]), px(SPLASH_WINDOW_SIZE[1])),
                cx,
            ))),
            titlebar: None,
            // Frameless splash: no OS titlebar/frame on any platform.
            // The in-window hover close button is the only chrome.
            window_decorations: Some(WindowDecorations::Client),
            is_resizable: false,
            window_min_size: Some(size(px(SPLASH_WINDOW_SIZE[0]), px(SPLASH_WINDOW_SIZE[1]))),
            ..Default::default()
        };
        gpui_kit::open_window(opts, cx, |window, cx| {
            cx.new(|cx| SplashPage::new(window, cx))
        })
        .is_ok()
    }
}

impl Render for SplashPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .flex()
            .flex_row()
            .overflow_hidden()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            // Left branding column: one third of the window width.
            // Left-dragging here moves the frameless window.
            .child(
                div()
                    .w(px(LEFT_WIDTH))
                    .flex_shrink_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .px_3()
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_move()
                    })
                    .child(
                        // Pre-rasterized wordmark PNG (see `build.rs`):
                        // same two-tone art as `sagex-varta.svg`, but
                        // a cheap decode instead of a vector raster.
                        // Native aspect 1207:500 preserved.
                        img("generated/wordmark.png").w(px(160.0)).h(px(66.0)),
                    )
                    .child(
                        // Edition + version on one row.
                        div()
                            .mt(px(12.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .font_family(theme::fonts::SANS)
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::rgb(theme::colors::ON_SURFACE))
                                    .child("Mayur Edition"),
                            )
                            .child(
                                div()
                                    .font_family(theme::fonts::SANS)
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::rgb(theme::colors::MUTED))
                                    .child("2026.10"),
                            ),
                    ),
            )
            // Right scenery panel: pre-rasterized PNG (see `build.rs`) is
            // cover-cropped to fill the remaining width; cropping a PNG is
            // cheap, unlike re-rasterizing the 4MB vector source.
            // Left-dragging here moves the frameless window.
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .relative()
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_move()
                    })
                    .child(
                        img("generated/scenery.png")
                            .w_full()
                            .h_full()
                            .object_fit(ObjectFit::Cover),
                    ),
            )
            // Team credit pinned to the window's bottom-left corner.
            .child(
                div()
                    .absolute()
                    .left(px(16.0))
                    .bottom(px(12.0))
                    .font_family(theme::fonts::SANS)
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child("Powered by Nivara"),
            )
            // Close button: invisible until hovered (opacity keeps it
            // hit-testable, so entering the corner reveals it). Solid chip
            // with a border so it reads as a button over the scenery.
            .child({
                let mut pos = div().absolute().top(px(10.0)).on_mouse_down(
                    MouseButton::Left,
                    |_, _, cx| cx.stop_propagation(),
                );
                pos = match self.close_side {
                    CloseSide::Left => pos.left(px(10.0)),
                    CloseSide::Right => pos.right(px(10.0)),
                };
                pos.child(
                    BaseButton::new(ElementId::Name("splash-close".into()))
                        .w(px(28.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(theme::radius::DEFAULT))
                        .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                        .border_1()
                        .border_color(theme::rgb(theme::colors::GRID_L2))
                        .opacity(0.0)
                        .hover(|s| s.opacity(1.0))
                        .on_click(|_, _, cx: &mut App| cx.quit())
                        .child(
                            svg()
                                .path("icons/x-mark.svg")
                                .w(px(14.0))
                                .h(px(14.0))
                                .text_color(theme::rgb(theme::colors::MUTED)),
                        ),
                )
            })
    }
}
