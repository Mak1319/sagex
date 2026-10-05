//! Schematic figure card, GLSL code block, attachment card.
use eframe::egui;
use crate::{icons::{self, Icon}, theme::*};
use super::primitives::{chip, mono};

pub fn pipeline(ui: &mut egui::Ui) {
    egui::Frame::NONE
        .fill(LOWEST)
        .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
        .corner_radius(2)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Network, 13.0, PRIMARY);
                ui.label(label("FIG 2.1 — LAYER BLEND PIPELINE [CAD SCHEMATIC]", 10.0, PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = ui.small_button("ACTIONS ▾");
                    chip(ui, "COLOR-SPACE: LINEAR sRGB", ON_VARIANT, LOW);
                });
            });
            ui.separator();
            // three responsive cells sharing the available width
            let third = ((ui.available_width() - 16.0) / 3.0).max(80.0);
            ui.horizontal(|ui| {
                for (h, v, f) in [
                    ("L0 : SOURCE [BASE]", "SRC_RGBA (8-bit)", "Alpha = 1.0"),
                    ("MULTIPLY (x)", "base x blend", "Norm clamped [0..1]"),
                    ("L1 : BLEND TARGET", "BLEND_BUFFER", "Factor = 0.854"),
                ] {
                    ui.allocate_ui_with_layout(
                        egui::vec2(third, 0.0),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            egui::Frame::NONE
                                .fill(LOW)
                                .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
                                .corner_radius(2)
                                .inner_margin(egui::Margin::same(6))
                                .show(ui, |ui| {
                                    ui.label(mono(ui, 8.5, h));
                                    ui.label(
                                        egui::RichText::new(v)
                                            .monospace()
                                            .size(9.5)
                                            .color(ON_SURFACE)
                                            .strong(),
                                    );
                                    ui.label(mono(ui, 8.5, f));
                                });
                        },
                    );
                }
            });
            ui.add_space(4.0);
            egui::Frame::NONE
                .fill(LOWEST)
                .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
                .corner_radius(2)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(label("CHANNEL VALUES MATRIX", 8.5, ON_VARIANT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(mono(ui, 8.5, "RGB (0.0 — 1.0)"));
                        });
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        for (c, name, val, col) in [
                            ("CH:", "RED", "(0.92)", egui::Color32::from_rgb(0xDC, 0x26, 0x26)),
                            ("CH:", "GRN", "(0.44)", SUCCESS),
                            ("CH:", "BLU", "(0.78)", PRIMARY_CTR),
                            ("RES:", "PASS-01", "", WARN),
                        ] {
                            ui.label(mono(ui, 9.5, c));
                            ui.label(egui::RichText::new(name).monospace().size(9.5).color(col).strong());
                            ui.label(mono(ui, 9.5, val));
                            ui.separator();
                        }
                    });
                });
        });
}

fn label(t: &str, s: f32, c: egui::Color32) -> egui::RichText {
    egui::RichText::new(t).monospace().size(s).color(c).strong()
}

pub fn code_block(ui: &mut egui::Ui) {
    egui::Frame::NONE
        .fill(LOWEST)
        .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
        .corner_radius(2)
        .inner_margin(egui::Margin::same(0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Code, 13.0, PRIMARY);
                ui.label(egui::RichText::new("// blend.glsl [glsl]").monospace().size(10.0).color(ON_SURFACE).strong());
                ui.label(mono(ui, 8.5, "(SHA-256: 8f4a...e12)"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    icons::show(ui, Icon::Copy, 12.0, PRIMARY);
                    ui.label(mono(ui, 8.5, "GLSL ES 3.0"));
                });
            });
            ui.separator();
            egui::ScrollArea::horizontal().show(ui, |ui| {
                let lines = [
                    ("01", "#version 300 es"),
                    ("02", "precision highp float;"),
                    ("03", ""),
                    ("04", "vec3 multiply(vec3 base, vec3 blend) {"),
                    ("05", "    return clamp(base * blend, 0.0, 1.0);"),
                    ("06", "}"),
                    ("07", "void main() {"),
                    ("08", "    vec4 c1 = texture(u_texture0, v_texCoord);"),
                    ("09", "    vec4 c2 = texture(u_texture1, v_texCoord);"),
                    ("10", "    fragColor = vec4(multiply(c1.rgb, c2.rgb), c1.a);"),
                    ("11", "}"),
                ];
                egui::Grid::new("glsl").striped(false).spacing([12.0, 2.0]).show(ui, |ui| {
                    for (no, code) in lines {
                        ui.label(egui::RichText::new(no).monospace().size(type_::CODE).color(OUTLINE_VAR));
                        ui.label(egui::RichText::new(code).monospace().size(type_::CODE).color(ON_SURFACE));
                        ui.end_row();
                    }
                });
            });
            egui::Frame::NONE.fill(ACTIVE_ROW).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("▲").monospace().size(10.0).color(PRIMARY).strong());
                    ui.label(egui::RichText::new("Shader for layer blending. Works with both color and single-channel grayscale input tensors without clipping.").monospace().size(10.0).color(ON_SURFACE));
                });
            });
        });
}

pub fn attachment(ui: &mut egui::Ui) {
    egui::Frame::NONE
        .fill(LOWEST)
        .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
        .corner_radius(2)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                egui::Frame::NONE.fill(ACTIVE_ROW).stroke(egui::Stroke::new(1.0, PRIMARY_FIXED)).corner_radius(2)
                    .inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                        icons::show(ui, Icon::MediaImage, 18.0, PRIMARY);
                    });
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("blend-comparison.png").size(12.5).color(ON_SURFACE).strong());
                    ui.label(mono(ui, 9.5, "PNG • 320 KB • 1024 x 683 px"));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    icons::show(ui, Icon::MoreVert, 14.0, OUTLINE);
                    icons::show(ui, Icon::Download, 14.0, OUTLINE);
                });
            });
        });
}
