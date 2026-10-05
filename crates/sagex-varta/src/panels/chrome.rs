//! Bottom status bar. (The top tech bar was removed: redundant chrome.)
use eframe::egui;
use crate::theme::*;
use crate::components::primitives::mono;

pub fn bottom(ui: &mut egui::Ui) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(mono(ui, 9.0, "SCALE: 1:1 | REV: 2.4 | X:142 Y:892 | GRID: 20mm INT | DIM 1440x900 | TOL ±0.005"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("PORT: 8080 // WS").monospace().size(9.0).color(PRIMARY).strong());
                ui.label(mono(ui, 9.0, "RECEPTOR: ONLINE"));
            });
        });
    });
}
