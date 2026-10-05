//! Widget adapter: schematic figure card.
use eframe::egui;
use crate::data::Message;
use super::{super::schematic as cards, body, ChatWidget};

pub struct SchematicWidget;
impl ChatWidget for SchematicWidget {
    fn show(&self, ui: &mut egui::Ui, msg: &Message) {
        body(ui, msg);
        ui.add_space(4.0);
        cards::pipeline(ui);
    }
}
