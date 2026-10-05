//! Widget adapter: GLSL code block card.
use eframe::egui;
use crate::data::Message;
use super::{super::schematic as cards, body, ChatWidget};

pub struct CodeWidget;
impl ChatWidget for CodeWidget {
    fn show(&self, ui: &mut egui::Ui, msg: &Message) {
        body(ui, msg);
        ui.add_space(4.0);
        cards::code_block(ui);
    }
}
