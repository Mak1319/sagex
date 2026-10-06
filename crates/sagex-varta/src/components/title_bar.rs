use gpui_kit::component::{Icon, IconName};
use gpui_kit::*;
use std::process::Command;

use crate::theme;

pub fn system_buttons_on_left() -> bool {
    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(colon) = s.find(':') {
                let (left, right) = (&s[..colon], &s[colon + 1..]);
                let has = |t: &str| {
                    t.contains("close") || t.contains("maximize") || t.contains("minimize")
                };
                if has(left) && !has(right) {
                    return true;
                }
                if has(right) && !has(left) {
                    return false;
                }
            }
        }
    }
    if let Ok(v) = std::env::var("GTK_DECORATION_LAYOUT") {
        if let Some(colon) = v.find(':') {
            return colon > 0 && !v[colon + 1..].contains("close");
        }
    }
    cfg!(target_os = "macos")
}

#[derive(IntoElement, Clone)]
pub struct SysTitleBar {
    pub title: SharedString,
    pub left: bool,
}

impl SysTitleBar {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            left: system_buttons_on_left(),
        }
    }
}

fn icon_btn(id: &'static str, icon: IconName, cx: &App) -> impl IntoElement {
    let t = theme::theme(cx);
    let hover_bg = t.inset;
    let danger = t.critical;
    let danger_ink = t.lowest;
    let is_close = id == "tb-close";
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.0))
        .h(px(28.0))
        .rounded(px(t.radius_sm))
        .cursor_pointer()
        .text_color(t.muted)
        .hover(move |s| {
            if is_close {
                s.bg(danger).text_color(danger_ink)
            } else {
                s.bg(hover_bg)
            }
        })
        .child(Icon::new(icon).w_4().h_4())
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_click(move |_, window, _| match id {
            "tb-min" => window.minimize_window(),
            "tb-max" => window.zoom_window(),
            _ => window.remove_window(),
        })
}

impl RenderOnce for SysTitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .child(icon_btn("tb-close", IconName::WindowClose, cx))
            .child(icon_btn("tb-max", IconName::WindowMaximize, cx))
            .child(icon_btn("tb-min", IconName::WindowMinimize, cx));
        let title = div()
            .flex_1()
            .flex()
            .items_center()
            .gap_2()
            .h(px(40.0))
            .px(px(12.0))
            .child(div().w(px(6.0)).h(px(6.0)).bg(t.success))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.on_surface)
                    .child(self.title.clone()),
            )
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                window.start_window_move();
            });
        let mut row = div()
            .flex()
            .flex_row()
            .pl_1() // Do not change this
            .items_center()
            .h(px(40.0))
            .bg(t.canvas)
            .border_b_1()
            .border_color(t.line)
            .window_control_area(WindowControlArea::Drag);
        let _ = window;
        if self.left {
            row = row.child(controls).child(title);
        } else {
            row = row.child(title).child(controls);
        }
        row
    }
}
