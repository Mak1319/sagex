//! Message card shell: avatar + header + widget-dispatched payload.
//!
//! Payload rendering is fully delegated to [`crate::components::widgets`]:
//! the card only handles identity chrome (avatar, author, time, role, LOC).
use eframe::egui;
use crate::{data::Message, theme::*};
use super::{
    primitives::{avatar, chip, corner_marks},
    widgets::render_payload,
};

fn role_chip(ui: &mut egui::Ui, m: &Message) {
    if let Some((r, fg, bg)) = m.role {
        chip(
            ui,
            r,
            egui::Color32::from_rgb(fg[0], fg[1], fg[2]),
            egui::Color32::from_rgb(bg[0], bg[1], bg[2]),
        );
    }
}

fn header(ui: &mut egui::Ui, m: &Message) {
    ui.horizontal_wrapped(|ui| {
        if m.own {
            role_chip(ui, m);
            ui.label(egui::RichText::new(m.time).monospace().size(9.5).color(OUTLINE));
            ui.label(
                egui::RichText::new(m.author)
                    .size(type_::BODY_MD)
                    .color(PRIMARY)
                    .strong(),
            );
        } else {
            ui.label(
                egui::RichText::new(m.author)
                    .size(type_::BODY_MD)
                    .color(ON_SURFACE)
                    .strong(),
            );
            ui.label(egui::RichText::new(m.time).monospace().size(9.5).color(OUTLINE));
            role_chip(ui, m);
        }
        ui.label(egui::RichText::new(m.loc).monospace().size(8.5).color(OUTLINE));
    });
}

pub fn message_card(ui: &mut egui::Ui, m: &Message) {
    let bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 102);
    let fg = egui::Color32::from_rgb(m.fg[0], m.fg[1], m.fg[2]);
    let bgc = egui::Color32::from_rgb(m.bg[0], m.bg[1], m.bg[2]);
    let resp = egui::Frame::NONE
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, OUTLINE_VAR))
        .corner_radius(2)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            // Linear layout only: avatar first, content fills the rest.
            // (A nested right_to_left block here previously claimed all
            // remaining width and collapsed the content column.)
            ui.horizontal(|ui| {
                if m.own {
                    ui.vertical(|ui| {
                        header(ui, m);
                        render_payload(ui, m);
                    });
                    avatar(ui, m.letter, bgc, fg, false);
                } else {
                    avatar(ui, m.letter, bgc, fg, false);
                    ui.vertical(|ui| {
                        header(ui, m);
                        render_payload(ui, m);
                    });
                }
            });
        })
        .response;
    corner_marks(ui.painter(), resp.rect);
}
