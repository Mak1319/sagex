//! Far-left 56px icon rail.
//!
//! Top: logo + nav. Bottom-anchored: tree-mode toggler, theme-mode
//! 3-state switch (dark/light/system), CFG, REV footer.
use eframe::egui;
use crate::{
    icons::{self, Icon},
    panels::left::LeftMode,
    theme::*,
};
use crate::components::primitives::panel_glyph;

const NAV: [(Icon, &str, bool); 4] = [
    (Icon::ChatBubble, "CHATS", true),
    (Icon::Network, "THRDS", false),
    (Icon::MultiplePages, "FILES", false),
    (Icon::Wrench, "TOOLS", false),
];

fn theme_button(ui: &mut egui::Ui, icon: Icon, active: bool, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
    if active {
        ui.painter().rect_filled(rect, 2.0, LOWEST);
        ui.painter().rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0, OUTLINE_VAR),
            egui::StrokeKind::Inside,
        );
    }
    ui.put(rect.shrink(5.0), icons::image(icon, 12.0, if active { PRIMARY } else { OUTLINE }));
    resp.on_hover_text(tip)
}

pub fn show(ui: &mut egui::Ui, mode: &mut LeftMode, theme: &mut ThemeMode) {
    egui::Panel::left("rail")
        .resizable(false)
        .default_size(RAIL_W)
        .show_separator_line(false)
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, LOW);
                ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, PRIMARY), egui::StrokeKind::Inside);
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "01", egui::FontId::monospace(12.0), PRIMARY);
                let dot = egui::pos2(r.max.x - 1.0, r.min.y + 1.0);
                ui.painter().circle_filled(dot, 4.0, LOWEST);
                ui.painter().circle_filled(dot, 3.0, PRIMARY);
                ui.add_space(4.0);
                ui.separator();
                for (icon, label, on) in NAV {
                    let mut f = egui::Frame::NONE.corner_radius(2).inner_margin(egui::Margin::symmetric(2, 6));
                    if on {
                        f = f.fill(PRIMARY_CTR).stroke(egui::Stroke::new(1.0, PRIMARY));
                    }
                    f.show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            icons::show(ui, icon, 17.0, if on { egui::Color32::WHITE } else { OUTLINE });
                            ui.label(egui::RichText::new(label).monospace().size(8.5).color(if on { egui::Color32::WHITE } else { OUTLINE }).strong());
                        });
                    });
                }
            });
            // ---- bottom-anchored block ----
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("CAL:OK").monospace().size(8.0).color(SUCCESS).strong());
                ui.label(egui::RichText::new("REV 2.4").monospace().size(8.0).color(PRIMARY).strong());
                ui.separator();
                icons::show(ui, Icon::Settings, 16.0, ON_VARIANT);
                ui.label(egui::RichText::new("CFG").monospace().size(8.0).color(ON_SURFACE).strong());
                ui.add_space(6.0);
                // theme 3-state switch
                egui::Frame::NONE
                    .fill(LOW)
                    .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
                    .corner_radius(2)
                    .inner_margin(egui::Margin::same(3))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            if theme_button(ui, Icon::HalfMoon, *theme == ThemeMode::Dark, "Dark").clicked() {
                                *theme = ThemeMode::Dark;
                            }
                            if theme_button(ui, Icon::SunLight, *theme == ThemeMode::Light, "Light").clicked() {
                                *theme = ThemeMode::Light;
                            }
                            if theme_button(ui, Icon::Computer, *theme == ThemeMode::System, "System").clicked() {
                                *theme = ThemeMode::System;
                            }
                        });
                    });
                ui.add_space(6.0);
                // tree-state mode toggler (painter glyph: font-safe)
                let ratio = match *mode {
                    LeftMode::Expanded => 1.0,
                    LeftMode::Dock => 0.3,
                    LeftMode::Hidden => 0.0,
                };
                let tname = match *mode {
                    LeftMode::Expanded => "TREE",
                    LeftMode::Dock => "DOCK",
                    LeftMode::Hidden => "HIDE",
                };
                let resp = egui::Frame::NONE
                    .fill(LOW)
                    .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
                    .corner_radius(2)
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            panel_glyph(ui, ratio);
                            ui.label(egui::RichText::new(tname).monospace().size(8.0).color(ON_SURFACE).strong());
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if resp.clicked() {
                    *mode = mode.next();
                }
                let _ = resp.on_hover_text("Left tree mode: Expanded > Dock > Hidden");
            });
        });
}
