//! Bottom message composer: mode tabs + telemetry + input + toolbar + send.
use eframe::egui;
use crate::{icons::Icon, theme::*};
use super::primitives::{chip, icon_button, mono};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ComposerTab {
    Message,
    Markdown,
}

pub struct Composer {
    pub text: String,
    pub tab: ComposerTab,
}

impl Composer {
    pub fn new() -> Self {
        Self { text: String::new(), tab: ComposerTab::Message }
    }

    fn tab_button(&mut self, ui: &mut egui::Ui, tab: ComposerTab, label: &str) {
        let active = self.tab == tab;
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(92.0, 24.0), egui::Sense::click());
        if active {
            ui.painter().rect_filled(rect, 2.0, LOWEST);
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0, OUTLINE_VAR),
                egui::StrokeKind::Inside,
            );
            // active tab underline
            ui.painter().line_segment(
                [
                    egui::pos2(rect.min.x + 4.0, rect.max.y - 1.0),
                    egui::pos2(rect.max.x - 4.0, rect.max.y - 1.0),
                ],
                egui::Stroke::new(2.0, PRIMARY),
            );
        }
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(9.5),
            if active { PRIMARY } else { OUTLINE },
        );
        if resp.clicked() {
            self.tab = tab;
        }
        let _ = resp.on_hover_text(label);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(LOWEST)
            .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
            .corner_radius(4)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.tab_button(ui, ComposerTab::Message, "MESSAGE");
                    self.tab_button(ui, ComposerTab::Markdown, "MARKDOWN");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        chip(ui, "GLSL/MD", ON_VARIANT, LOW);
                        ui.label(mono(ui, 9.5, "SYNTAX:"));
                    });
                });
                ui.separator();
                // full-width input
                ui.add(
                    egui::TextEdit::multiline(&mut self.text)
                        .hint_text("Type technical parameter or command...")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.separator();
                ui.horizontal(|ui| {
                    for (ic, tip) in [
                        (Icon::Attachment, "Attach CAD file"),
                        (Icon::MediaImage, "Insert raster image"),
                        (Icon::Code, "Insert code block"),
                    ] {
                        icon_button(ui, ic, 14.0, OUTLINE, tip);
                        ui.add_space(4.0);
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("B").size(11.0).color(ON_SURFACE).strong());
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("I").size(11.0).color(ON_SURFACE).italics());
                    ui.add_space(4.0);
                    ui.separator();
                    for (ic, tip) in [
                        (Icon::Bookmark, "Bookmark"),
                        (Icon::Quote, "Quote"),
                        (Icon::Forward, "Forward"),
                    ] {
                        icon_button(ui, ic, 14.0, OUTLINE, tip);
                        ui.add_space(4.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // SEND first (rightmost) so it can never be clipped
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(86.0, 28.0),
                            egui::Sense::click(),
                        );
                        ui.painter().rect_filled(rect, 4.0, PRIMARY);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "SEND",
                            egui::FontId::monospace(10.0),
                            egui::Color32::WHITE,
                        );
                        let _ = resp.on_hover_text("Transmit (Enter)");
                        ui.label(mono(ui, 8.5, "Enter to send"));
                    });
                });
            });
    }
}
