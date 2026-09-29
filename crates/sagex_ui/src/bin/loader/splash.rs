//! Wizard splash demo: two-panel dialog in the reference layout — white
//! brand panel (logo, name, version, progress, status, lock row) beside a
//! full-bleed flat illustration with a small credit line.
//!
//! The animation is configured **through code** via [`SplashConfig`]
//! (duration, end behavior, hold, bar color, title, version, tagline,
//! credit). No CLI, no stdin.

use std::time::{Duration, Instant};

use gpui::{
    App, AssetSource, Context, Entity, IntoElement, ParentElement, Render, Result, SharedString,
    Styled, Timer, Window, div, px, relative, rgb, svg,
};
use gpui_component::{Icon, StyledExt as _};
use std::borrow::Cow;

/// Fixed dialog palette (reference look: white panel, ink text).
const PANEL: u32 = 0xffffff;
const INK: u32 = 0x1a1a1a;
const MUTED: u32 = 0x6b7280;
const TRACK: u32 = 0xe5e7eb;

/// What the demo does when the bar fills. All variants are valid
/// `SplashConfig` choices (selected in `loader.rs`).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplashEnd {
    /// Freeze on the finished splash.
    #[default]
    Hold,
    /// Pause, then replay the fill.
    Loop,
    /// Close the window, exit 0.
    Exit,
}

/// Code-configurable animation. Tweak these constants and rebuild.
#[derive(Debug, Clone)]
pub struct SplashConfig {
    /// 0→100% fill time.
    pub duration: Duration,
    /// Behavior once full.
    pub end: SplashEnd,
    /// Pause on full before `Loop` restarts or `Exit` quits.
    pub hold: Duration,
    /// Bar fill color (default sagex accent green).
    pub bar: u32,
    /// Wordmark next to the logo.
    pub title: &'static str,
    /// Small version line under the title.
    pub version: &'static str,
    /// Line next to the lock glyph.
    pub tagline: &'static str,
    /// Small print over the artwork's bottom-right corner.
    pub credit: &'static str,
}

impl Default for SplashConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_millis(2600),
            end: SplashEnd::Hold,
            hold: Duration::from_millis(900),
            bar: 0x00a884,
            title: "sagex",
            version: "sagex // 0.1.0",
            tagline: "End-to-end encrypted",
            credit: "Post-quantum secure",
        }
    }
}

/// Minimal embedded assets (lock + logo mark) for this standalone binary.
pub struct SplashAssets;

impl AssetSource for SplashAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "icons/lock.svg" => Some(include_str!("assets/lock.svg").as_bytes()),
            "icons/gallery-vertical-end.svg" => {
                Some(include_str!("assets/gallery-vertical-end.svg").as_bytes())
            }
            "art/splash-art.svg" => Some(include_str!("assets/splash-art.svg").as_bytes()),
            _ => None,
        };
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        if path == "icons" || path == "icons/" || path.is_empty() {
            Ok(vec![
                "icons/lock.svg".into(),
                "icons/gallery-vertical-end.svg".into(),
            ])
        } else if path == "art" || path == "art/" {
            Ok(vec!["art/splash-art.svg".into()])
        } else {
            Ok(vec![])
        }
    }
}

pub struct SplashView {
    cfg: SplashConfig,
    progress: f32, // 0.0..=1.0
    status: String,
    started: Instant,
    holding_since: Option<Instant>,
}

impl SplashView {
    pub fn new(cfg: SplashConfig) -> Self {
        Self {
            cfg,
            progress: 0.0,
            status: "Starting…".to_string(),
            started: Instant::now(),
            holding_since: None,
        }
    }

    /// 60fps tick driver. Stops itself when the configured end is reached.
    pub fn boot(view: &Entity<Self>, cx: &mut App) {
        let view = view.clone();
        cx.spawn(async move |cx| {
            loop {
                Timer::after(Duration::from_millis(16)).await;
                let alive = view
                    .update(cx, |v, cx| {
                        v.tick();
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }

    fn tick(&mut self) {
        if let Some(since) = self.holding_since {
            if since.elapsed() >= self.cfg.hold {
                match self.cfg.end {
                    SplashEnd::Loop => {
                        self.started = Instant::now();
                        self.holding_since = None;
                        self.progress = 0.0;
                    }
                    SplashEnd::Exit => std::process::exit(0),
                    SplashEnd::Hold => {}
                }
            }
            return;
        }
        let t = (self.started.elapsed().as_secs_f32() / self.cfg.duration.as_secs_f32()).min(1.0);
        // ease-in-out cubic, like a boot shimmer
        self.progress = if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        };
        // wizard phase line follows the fill
        self.status = match (self.progress * 100.0) as u32 {
            0..=24 => "Connecting to server…".to_string(),
            25..=54 => "Restoring session…".to_string(),
            55..=84 => "Loading rooms…".to_string(),
            _ => "Opening chat…".to_string(),
        };
        if t >= 1.0 {
            self.progress = 1.0;
            self.status = "Ready".to_string();
            self.holding_since = Some(Instant::now());
            if self.cfg.end == SplashEnd::Exit && self.cfg.hold.is_zero() {
                std::process::exit(0);
            }
        }
    }
}

impl Render for SplashView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let bar_w = relative(self.progress.clamp(0.0, 1.0));
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(PANEL))
            // ---- left brand panel ----
            .child(
                div()
                    .w(px(340.))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .bg(rgb(PANEL))
                    .p(px(36.))
                    .child(
                        // logo row: accent mark + title + version
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(
                                Icon::empty()
                                    .path("icons/gallery-vertical-end.svg")
                                    .size(px(40.))
                                    .text_color(rgb(self.cfg.bar)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_bold()
                                            .text_color(rgb(INK))
                                            .child(self.cfg.title),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(MUTED))
                                            .child(self.cfg.version),
                                    ),
                            ),
                    )
                    .child(
                        // bottom wizard block: status, bar, lock row
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .truncate()
                                    .text_color(rgb(MUTED))
                                    .child(self.status.clone()),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(4.))
                                    .rounded_full()
                                    .bg(rgb(TRACK))
                                    .child(
                                        div()
                                            .h_full()
                                            .w(bar_w)
                                            .rounded_full()
                                            .bg(rgb(self.cfg.bar)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::empty()
                                            .path("icons/lock.svg")
                                            .size(px(14.))
                                            .text_color(rgb(MUTED)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(MUTED))
                                            .child(self.cfg.tagline),
                                    ),
                            ),
                    ),
            )
            // ---- right full-bleed illustration ----
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w(px(0.))
                    .relative()
                    .child(svg().path("art/splash-art.svg").w_full().h_full())
                    .child(
                        div()
                            .absolute()
                            .bottom(px(14.))
                            .right(px(16.))
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .child(self.cfg.credit),
                    ),
            )
            .into_any_element()
    }
}
