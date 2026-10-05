//! Widget adapter: file attachment card.
use eframe::egui;
use crate::data::Message;
use super::{super::schematic as cards, body, ChatWidget};

pub struct AttachmentWidget;
impl ChatWidget for AttachmentWidget {
    fn show(&self, ui: &mut egui::Ui, msg: &Message) {
        body(ui, msg);
        ui.add_space(4.0);
        cards::attachment(ui);
    }
}
