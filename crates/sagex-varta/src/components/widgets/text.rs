//! Plain body text + question action row (react / reply / more).
use eframe::egui;
use crate::{data::Message, icons::{self, Icon}, theme::*};
use super::{body, ChatWidget};

pub struct QuestionWidget;

impl ChatWidget for QuestionWidget {
    fn show(&self, ui: &mut egui::Ui, msg: &Message) {
        body(ui, msg);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("▲").size(10.0).color(OUTLINE));
            ui.label(egui::RichText::new("▲").size(10.0).color(OUTLINE));
            ui.separator();
            icons::show(ui, Icon::Reply, 13.0, OUTLINE);
            icons::show(ui, Icon::MoreHoriz, 13.0, OUTLINE);
        });
    }
}
