//! Center: channel header + axis ruler + hatch band + message stream + composer.
//!
//! Fully fluid: no fixed content widths — cells share `available_width()`.
//! Thin splitter gutters are painted on both panel edges (hover = primary)
//! while the panels themselves hide their default separator lines.
use eframe::egui;
use crate::{data, icons::{self, Icon}, theme::*};
use crate::components::{
    composer::Composer,
    message_card::message_card,
    primitives::{avatar, axis_ruler, blueprint_grid, chip, hatch_strip, mono},
};

pub struct CenterState {
    pub composer: Composer,
}

/// 5px splitter gutter painted on a panel edge: 1px line + 3 grip dots.
fn splitter_gutter(ui: &mut egui::Ui, edge: egui::Rect, at_left: bool) {
    let p = ui.painter();
    let x = if at_left { edge.min.x + 2.0 } else { edge.max.x - 2.0 };
    let hover = p.ctx()
        .pointer_hover_pos()
        .map(|pos| (pos.x - x).abs() < 6.0 && edge.y_range().contains(pos.y))
        .unwrap_or(false);
    let col = if hover { PRIMARY_CTR } else { OUTLINE_VAR };
    p.line_segment(
        [egui::pos2(x, edge.min.y), egui::pos2(x, edge.max.y)],
        egui::Stroke::new(1.0, col),
    );
    let cy = edge.center().y;
    for dy in [-8.0, 0.0, 8.0] {
        p.circle_filled(egui::pos2(x, cy + dy), 1.2, col);
    }
}

pub fn show(ui: &mut egui::Ui, st: &mut CenterState) {
    egui::CentralPanel::default().show(ui, |ui| {
        let edge = ui.max_rect();
        // header 48px
        ui.horizontal(|ui| {
            avatar(ui, "I", PRIMARY_FIXED, PRIMARY, true);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading("Image Processing");
                    chip(ui, "ID: #0x88F", OUTLINE, LOW);
                });
                ui.horizontal(|ui| {
                    ui.label(mono(ui, 9.5, "5 nodes"));
                    ui.label(mono(ui, 9.5, "•"));
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("●").size(6.0).color(SUCCESS));
                        ui.label(egui::RichText::new("3 online").monospace().size(9.5).color(SUCCESS));
                    });
                    ui.label(mono(ui, 9.5, "•"));
                    ui.label(egui::RichText::new("TOL: 0.001").monospace().size(9.5).color(PRIMARY).strong());
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for ic in [Icon::VideoCamera, Icon::Group, Icon::Search, Icon::MoreVert] {
                    icons::show(ui, ic, 16.0, OUTLINE);
                }
                chip(ui, "| FLUID CAD VIEWPORT |", OUTLINE, LOW);
            });
        });
        axis_ruler(ui);
        hatch_strip(ui, 22.0);
        // stream: bounded so the composer below always stays visible
        let scroll_h = (ui.available_height() - 190.0).max(120.0);
        egui::ScrollArea::vertical().max_height(scroll_h).show(ui, |ui| {
            blueprint_grid(ui.painter(), ui.clip_rect());
            ui.horizontal(|ui| {
                ui.with_layout(
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        chip(ui, "● TIMESTEP: OCT 04, 2026 | T_ZERO: 14:12 UTC", ON_SURFACE, LOWEST);
                    },
                );
            });
            for m in data::messages() {
                message_card(ui, &m);
                ui.add_space(8.0);
            }
        });
        ui.separator();
        st.composer.show(ui);
        // splitter gutters painted last (on top, at the live edges)
        let full = edge;
        splitter_gutter(ui, full, true);
        splitter_gutter(ui, full, false);
    });
}
