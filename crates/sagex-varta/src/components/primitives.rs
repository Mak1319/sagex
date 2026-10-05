//! Small reusable CAD primitives: chips, avatars, grids, rulers, section bars.
use eframe::egui;
use crate::{icons::{self, Icon}, theme::*};

/// bordered mono micro-chip
pub fn chip(ui: &mut egui::Ui, text: &str, fg: egui::Color32, bg: egui::Color32) {
    egui::Frame::NONE
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
        .corner_radius(2)
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .monospace()
                    .size(type_::LABEL_SM)
                    .color(fg)
                    .strong(),
            );
        });
}

pub fn mono(ui: &egui::Ui, size: f32, text: &str) -> egui::RichText {
    let _ = ui;
    egui::RichText::new(text).monospace().size(size).color(OUTLINE)
}

pub fn label(text: &str, size: f32, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text).monospace().size(size).color(color).strong()
}

/// 28px tinted avatar square with presence dot option
pub fn avatar(ui: &mut egui::Ui, letter: &str, bg: egui::Color32, fg: egui::Color32, presence: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(AVATAR, AVATAR), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, bg);
    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, fg), egui::StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::monospace(11.0),
        fg,
    );
    if presence {
        let c = rect.right_bottom() + egui::vec2(-1.0, -1.0);
        ui.painter().circle_filled(c, 4.0, LOWEST);
        ui.painter().circle_filled(c, 3.0, SUCCESS);
    }
}

/// blue L corner marks (6px)
pub fn corner_marks(painter: &egui::Painter, rect: egui::Rect) {
    let s = egui::Stroke::new(1.5, PRIMARY_CTR);
    let l = 6.0;
    let p = rect.min + egui::vec2(3.0, 3.0);
    painter.line_segment([p, p + egui::vec2(l, 0.0)], s);
    painter.line_segment([p, p + egui::vec2(0.0, l)], s);
    let q = rect.max - egui::vec2(3.0, 3.0);
    painter.line_segment([q, q - egui::vec2(l, 0.0)], s);
    painter.line_segment([q, q - egui::vec2(0.0, l)], s);
}

/// dotted blueprint grid
pub fn blueprint_grid(painter: &egui::Painter, rect: egui::Rect) {
    let dot = egui::Color32::from_rgba_unmultiplied(0x25, 0x63, 0xEB, 0x18);
    let mut x = rect.min.x;
    while x <= rect.max.x {
        let mut y = rect.min.y;
        while y <= rect.max.y {
            painter.circle_filled(egui::pos2(x, y), 1.0, dot);
            y += 20.0;
        }
        x += 20.0;
    }
}

/// 6px axis ruler strip (20px gray ticks, 100px blue ticks)
pub fn axis_ruler(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 0.0, LOWEST);
    let mut x = r.min.x;
    let mut i = 0;
    while x < r.max.x {
        let major = i % 5 == 0;
        ui.painter().line_segment(
            [egui::pos2(x, r.min.y), egui::pos2(x, r.max.y)],
            egui::Stroke::new(if major { 1.5 } else { 1.0 }, if major { PRIMARY_CTR } else { OUTLINE_VAR }),
        );
        x += 20.0;
        i += 1;
    }
}

/// section bar: icon + title left, meta right
pub fn section_bar(ui: &mut egui::Ui, icon: Icon, title: &str, meta: &str) {
    egui::Frame::NONE
        .fill(LOW)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icons::show(ui, icon, 12.0, PRIMARY);
                ui.label(label(title, type_::LABEL_SM, ON_SURFACE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(meta).monospace().size(8.5).color(OUTLINE),
                    );
                });
            });
        });
}

/// bordered card frame (white, 1px outline-variant, 4px radius)
pub fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(LOWEST)
        .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
        .corner_radius(4)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| add_contents(ui))
        .inner
}

/// card header row: small icon + caps title left, meta right
pub fn card_header(ui: &mut egui::Ui, icon: Icon, title: &str, meta: &str) {
    ui.horizontal(|ui| {
        icons::show(ui, icon, 12.0, PRIMARY);
        ui.label(label(title, type_::LABEL_SM, ON_SURFACE));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(meta).monospace().size(8.5).color(OUTLINE).strong());
        });
    });
    ui.separator();
}

/// Left-truncated label + right-pinned trailing content that can never be
/// pushed out of view. `right_w` reserves space for the trailing side.
pub fn trailing_row<R>(
    ui: &mut egui::Ui,
    left: egui::RichText,
    right_w: f32,
    right: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let avail = ui.available_width();
    let lw = (avail - right_w - 8.0).max(40.0);
    let mut out = None;
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::vec2(lw, 18.0),
            egui::Label::new(left).truncate(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            out = Some(right(ui));
        });
    });
    out.unwrap()
}

/// 28×28 icon button with hover fill. Returns clicked response.
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, size: f32, color: egui::Color32, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, ACTIVE_ROW);
    }
    ui.put(rect.shrink(6.0), icons::image(icon, size, color));
    resp.on_hover_text(tip)
}

/// Presence dot drawn with the painter (no font glyph dependency).
/// Hollow (away/offline) draws an outline ring; solid otherwise.
pub fn presence_dot(ui: &mut egui::Ui, color: egui::Color32, hollow: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    let c = rect.center();
    if hollow {
        ui.painter().circle_stroke(c, 3.0, egui::Stroke::new(1.5, color));
    } else {
        ui.painter().circle_filled(c, 3.5, color);
    }
}

/// Mini tree-mode glyph drawn with the painter (font-safe):
/// full sidebar / slim dock / hidden (outline only).
pub fn panel_glyph(ui: &mut egui::Ui, filled_ratio: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 14.0), egui::Sense::hover());
    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.2, PRIMARY), egui::StrokeKind::Inside);
    if filled_ratio > 0.01 {
        let w = rect.width() * filled_ratio;
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(w, rect.height()));
        ui.painter().rect_filled(fill, 2.0, PRIMARY_CTR);
    }
}

/// Light hatched divider strip (diagonal etch + top/bottom borders +
/// vertical joints at thirds), echoing the reference drafting band.
pub fn hatch_strip(ui: &mut egui::Ui, height: f32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 0.0, LOW);
    let edge = egui::Stroke::new(1.0, OUTLINE_VAR);
    p.line_segment([r.left_top(), r.right_top()], edge);
    p.line_segment([r.left_bottom(), r.right_bottom()], edge);
    let hatch = egui::Stroke::new(1.0, CONTAINER);
    let mut x = r.min.x - height;
    while x < r.max.x {
        let a = egui::pos2(x.max(r.min.x), r.max.y);
        let b = egui::pos2((x + height).min(r.max.x), r.min.y);
        // only draw segments fully inside
        if a.x <= r.max.x && b.x >= r.min.x && a.x < b.x {
            p.line_segment([a, b], hatch);
        }
        x += 9.0;
    }
    for t in [0.3333, 0.6667] {
        let x = r.min.x + r.width() * t;
        p.line_segment([egui::pos2(x, r.min.y), egui::pos2(x, r.max.y)], edge);
    }
}

/// kbd hint chip with ASCII-only text (font-safe).
pub fn kbd(ui: &mut egui::Ui, text: &str) {
    egui::Frame::NONE
        .fill(LOWEST)
        .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
        .corner_radius(2)
        .inner_margin(egui::Margin::symmetric(4, 0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).monospace().size(8.5).color(OUTLINE).strong());
        });
}
