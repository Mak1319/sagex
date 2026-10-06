use std::rc::Rc;

use gpui_kit::{component::Icon, prelude::*, *};

use crate::theme::{self, TintColor};

/// Monospace uppercase telemetry tag (role chips, ID chips, badges).
#[derive(IntoElement, Clone)]
pub struct MetaChip {
    label: SharedString,
    tint: TintColor,
}

impl MetaChip {
    pub fn new(label: impl Into<SharedString>, tint: TintColor) -> Self {
        Self {
            label: label.into(),
            tint,
        }
    }
}

impl RenderOnce for MetaChip {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        let tint = t.tint(self.tint);
        div()
            .px(px(6.0))
            .rounded(px(t.radius_sm))
            .bg(tint.bg)
            .text_color(tint.fg)
            .border_1()
            .border_color(tint.border)
            .text_size(px(8.5))
            .font_weight(FontWeight::BOLD)
            .child(self.label)
    }
}

/// 6px round status ping (online green, alert red, idle amber).
#[derive(IntoElement, Clone)]
pub struct PresenceDot {
    color: Rgba,
}

impl PresenceDot {
    pub fn new(color: Rgba) -> Self {
        Self { color }
    }

    pub fn online(cx: &App) -> Self {
        Self::new(theme::theme(cx).success)
    }
}

impl RenderOnce for PresenceDot {
    fn render(self, _w: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex_none()
            .w(px(6.0))
            .h(px(6.0))
            .rounded_full()
            .bg(self.color)
    }
}

/// Fixed-size line icon from `assets/icons/` (Iconoir → Hugeicons → Lucide).
#[derive(IntoElement, Clone)]
pub struct KitIcon {
    path: SharedString,
    size: f32,
    color: Option<Rgba>,
}

impl KitIcon {
    pub fn new(path: impl Into<SharedString>, size: f32) -> Self {
        Self {
            path: path.into(),
            size,
            color: None,
        }
    }

    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for KitIcon {
    fn render(self, _w: &mut Window, _cx: &mut App) -> impl IntoElement {
        let icon = Icon::empty()
            .path(self.path.clone())
            .w(px(self.size))
            .h(px(self.size));
        match self.color {
            Some(color) => icon.text_color(color),
            None => icon,
        }
    }
}

/// Square icon button (header actions, composer tools). One struct for
/// all sizes — no per-site helper functions.
#[derive(IntoElement, Clone)]
pub struct IconBtn {
    path: &'static str,
    icon: f32,
    box_size: f32,
    color: Option<Rgba>,
    action_id: Option<&'static str>,
    on_action: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl IconBtn {
    pub fn new(path: &'static str, icon: f32, box_size: f32) -> Self {
        Self {
            path,
            icon,
            box_size,
            color: None,
            action_id: None,
            on_action: None,
        }
    }

    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Click handler. Assigns a stable id (required for statefulness
    /// in this GPUI version) and wires `on_click`.
    pub fn on_action(
        mut self,
        id: &'static str,
        f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action_id = Some(id);
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconBtn {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme::theme(cx);
        let color = self.color.unwrap_or(t.muted);
        let body = KitIcon::new(self.path, self.icon).color(color);
        // `.id()` changes the element type (Div → Stateful<Div>), so the
        // clickable branch is built separately instead of chained.
        match (self.action_id, self.on_action) {
            (Some(id), Some(f)) => div()
                .id(SharedString::from(format!("tool-{id}")))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .w(px(self.box_size))
                .h(px(self.box_size))
                .rounded(px(t.radius_sm))
                .cursor_pointer()
                .on_click(move |ev, window, cx| f(ev, window, cx))
                .child(body)
                .into_any_element(),
            _ => div()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .w(px(self.box_size))
                .h(px(self.box_size))
                .rounded(px(t.radius_sm))
                .child(body)
                .into_any_element(),
        }
    }
}
