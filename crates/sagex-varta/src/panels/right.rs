//! Right CAD property sheet — card-based inspector (see reference).
//!
//! Grid contract mirror of the left panel: default 300px, dragging below
//! 300px collapses to the dock. Explicit state only.
use eframe::egui;
use crate::{data, icons::{self, Icon}, theme::*};
use crate::components::primitives::{avatar, card, card_header, chip, label, mono, presence_dot, trailing_row};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RightMode {
    /// >=300px inspector (resizable up to 380).
    Expanded,
    /// 36px slim dock with a reopen button.
    Dock,
}

pub struct RightState {
    pub mode: RightMode,
}

impl RightState {
    pub fn new() -> Self {
        Self { mode: RightMode::Expanded }
    }
}

fn prop_row(ui: &mut egui::Ui, key: &str, value: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).monospace().size(9.5).color(OUTLINE));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).monospace().size(9.5).color(color).strong());
        });
    });
    ui.separator();
}

fn status_chip(ui: &mut egui::Ui, text: &str, fg: egui::Color32, bg: egui::Color32) {
    chip(ui, text, fg, bg);
}

pub fn show(ui: &mut egui::Ui, st: &mut RightState) {
    if st.mode == RightMode::Dock {
        egui::Panel::right("right_dock")
            .resizable(false)
            .default_size(DOCK_W)
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    if ui.small_button("«").on_hover_text("Show inspector").clicked() {
                        st.mode = RightMode::Expanded;
                    }
                    ui.label(label("INSPECT", 8.0, PRIMARY));
                });
            });
        return;
    }
    let resp = egui::Panel::right("right_props")
        .resizable(true)
        .default_size(RIGHT_W)
        .min_size(DOCK_W)
        .max_size(380.0)
        .show_separator_line(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Frame, 14.0, PRIMARY);
                ui.label(label("CAD // PROPERTY SHEET / INSPECT", type_::LABEL_SM, ON_SURFACE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("»").on_hover_text("Collapse to dock").clicked() {
                        st.mode = RightMode::Dock;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // channel identity card
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        avatar(ui, "I", PRIMARY_FIXED, PRIMARY, false);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Image Processing")
                                    .size(15.0)
                                    .color(ON_SURFACE)
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new("#dwg-image-processing")
                                    .monospace()
                                    .size(10.0)
                                    .color(PRIMARY),
                            );
                        });
                    });
                    ui.separator();
                    ui.label(
                        egui::RichText::new(
                            "Discussion about image processing algorithms, blending modes, filters and computer vision.",
                        )
                        .size(11.5)
                        .color(ON_VARIANT),
                    );
                });
                ui.add_space(8.0);
                // element properties table card
                card(ui, |ui| {
                    card_header(ui, Icon::Database, "ELEMENT PROPERTIES", "SEC-04");
                    prop_row(ui, "CREATED_STAMP", "2026-08-12", ON_SURFACE);
                    prop_row(ui, "ACTIVE_MEMBERS", "5 ACTIVE", SUCCESS);
                    prop_row(ui, "MESSAGE_COUNT", "342 PACKETS", PRIMARY);
                    prop_row(ui, "LOG_RETENTION", "90 DAYS (LOCKED)", WARN);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("FRAME_RATE").monospace().size(9.5).color(OUTLINE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("60.00 FPS").monospace().size(9.5).color(ON_SURFACE).strong());
                        });
                    });
                });
                ui.add_space(8.0);
                ui.label(label("TAG REGISTRY", type_::LABEL_SM, OUTLINE));
                ui.horizontal_wrapped(|ui| {
                    chip(ui, "rendering", PRIMARY, ACTIVE_ROW);
                    for t in ["shaders", "cv", "imgproc"] {
                        chip(ui, t, ON_VARIANT, LOW);
                    }
                });
                ui.add_space(8.0);
                // operators card
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        icons::show(ui, Icon::Group, 12.0, PRIMARY);
                        ui.label(label("OPERATORS (5)", type_::LABEL_SM, ON_SURFACE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new("3 ONLINE").monospace().size(8.5).color(SUCCESS).strong(),
                            );
                        });
                    });
                    ui.separator();
                    for (name, tag, fg, bg) in [
                        ("Arjun Mehta", "OWNER", PRIMARY, ACTIVE_ROW),
                        ("Priya Sharma", "ONLINE", SUCCESS, SUCCESS_BG),
                        ("You", "ONLINE", SUCCESS, SUCCESS_BG),
                        ("Mira Patel", "AWAY", WARN, WARN_BG),
                        ("Karan Varma", "OFFLINE", OUTLINE, LOW),
                    ] {
                        let hollow = tag == "AWAY" || tag == "OFFLINE";
                        let ink = if name == "You" { PRIMARY } else { ON_SURFACE };
                        ui.horizontal(|ui| {
                            presence_dot(ui, if hollow { OUTLINE } else { SUCCESS }, hollow);
                            trailing_row(
                                ui,
                                egui::RichText::new(name).size(11.5).color(ink).strong(),
                                66.0,
                                |ui| status_chip(ui, tag, fg, bg),
                            );
                        });
                        ui.separator();
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(label("SUB-SCHEMATICS (2)", type_::LABEL_SM, OUTLINE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        icons::show(ui, Icon::ChatBubble, 12.0, OUTLINE);
                    });
                });
                for (title, replies, age, selected, count) in [
                    ("Blend mode discussion", "4 replies", "2h ago", true, "3"),
                    ("Grayscale support", "1 reply", "1d ago", false, "1"),
                ] {
                    let mut f = egui::Frame::NONE
                        .fill(LOWEST)
                        .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
                        .corner_radius(4)
                        .inner_margin(egui::Margin::same(8));
                    if selected {
                        f = f.stroke(egui::Stroke::new(1.5, PRIMARY_CTR));
                    }
                    f.show(ui, |ui| {
                        trailing_row(
                            ui,
                            egui::RichText::new(title).size(12.0).color(ON_SURFACE).strong(),
                            92.0,
                            |ui| {
                                ui.label(mono(ui, 9.0, age));
                                if selected {
                                    egui::Frame::NONE
                                        .fill(PRIMARY)
                                        .corner_radius(2)
                                        .inner_margin(egui::Margin::symmetric(6, 0))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(count)
                                                    .monospace()
                                                    .size(9.0)
                                                    .color(egui::Color32::WHITE)
                                                    .strong(),
                                            );
                                        });
                                }
                            },
                        );
                        ui.label(mono(ui, 9.5, replies));
                    });
                    ui.add_space(4.0);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(label("DWG ATTACHMENTS (3)", type_::LABEL_SM, OUTLINE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(mono(ui, 8.5, "4.8 MB"));
                    });
                });
                for (name, size) in data::attachments() {
                    card(ui, |ui| {
                        ui.horizontal(|ui| {
                            icons::show(ui, Icon::Database, 14.0, PRIMARY);
                            trailing_row(
                                ui,
                                egui::RichText::new(name).size(11.5).color(ON_SURFACE).strong(),
                                76.0,
                                |ui| {
                                    icons::show(ui, Icon::Download, 14.0, OUTLINE);
                                    ui.label(mono(ui, 9.0, size));
                                },
                            );
                        });
                    });
                    ui.add_space(4.0);
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("●").size(8.0).color(SUCCESS));
                    ui.label(
                        egui::RichText::new("CONNECTED · LATENCY: 28 ms · v0.1.0")
                            .monospace()
                            .size(9.0)
                            .color(SUCCESS)
                            .strong(),
                    );
                });
                ui.label(mono(ui, 8.5, "LAST SYNC: 14:20:31 UTC"));
            });
        })
        .response;
    // drag-below-min collapses to the dock (dock is non-resizable: stable)
    if resp.rect.width() < PANEL_MIN {
        st.mode = RightMode::Dock;
    }
}
