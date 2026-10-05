//! Conversation list row: avatar + name/time + preview/badge.
//!
//! Both text lines use [`trailing_row`] with reserved trailing widths so
//! timestamps and badges can never be pushed out of the panel.
use eframe::egui;
use crate::{data::Node, icons::{self, Icon}, theme::*};
use super::primitives::{avatar, corner_marks, trailing_row};

pub fn node_row(ui: &mut egui::Ui, n: &Node) {
    let bg = if n.active { ACTIVE_ROW } else { LOWEST };
    let resp = egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_min_height(ROW_H - 12.0);
            ui.horizontal(|ui| {
                avatar(ui, n.letter, n.bg, n.fg, n.active);
                ui.vertical(|ui| {
                    trailing_row(
                        ui,
                        egui::RichText::new(n.name)
                            .size(type_::BODY_SM + 0.5)
                            .color(if n.active { PRIMARY } else { ON_SURFACE })
                            .strong(),
                        64.0,
                        |ui| {
                            ui.label(
                                egui::RichText::new(n.time)
                                    .monospace()
                                    .size(9.5)
                                    .color(if n.active { PRIMARY } else { OUTLINE }),
                            );
                        },
                    );
                    if let Some(b) = n.badge {
                        trailing_row(
                            ui,
                            egui::RichText::new(n.preview).size(11.0).color(ON_VARIANT),
                            30.0,
                            |ui| {
                                egui::Frame::NONE
                                    .fill(PRIMARY)
                                    .corner_radius(2)
                                    .inner_margin(egui::Margin::symmetric(6, 0))
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(b)
                                                .monospace()
                                                .size(9.0)
                                                .color(egui::Color32::WHITE)
                                                .strong(),
                                        );
                                    });
                            },
                        );
                    } else {
                        trailing_row(
                            ui,
                            egui::RichText::new(n.preview).size(11.0).color(ON_VARIANT),
                            20.0,
                            |ui| {
                                if n.name == "UI/UX Design" {
                                    icons::show(ui, Icon::Attachment, 12.0, OUTLINE);
                                } else if n.name == "Build & DevOps" {
                                    ui.label(egui::RichText::new("●").size(6.0).color(ERROR));
                                }
                            },
                        );
                    }
                });
                if n.active {
                    icons::show(ui, Icon::MoreVert, 14.0, OUTLINE);
                }
            });
        })
        .response;
    if n.active {
        let r = resp.rect;
        ui.painter().line_segment(
            [r.min, egui::pos2(r.min.x, r.max.y)],
            egui::Stroke::new(2.0, PRIMARY),
        );
        corner_marks(ui.painter(), r);
    }
}
