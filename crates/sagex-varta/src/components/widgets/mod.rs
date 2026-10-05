//! Multi-type chat widget system.
//!
//! Every message payload renders through a [`ChatWidget`]. New widget kinds
//! (polls, diffs, plots, tool-calls…) plug in by implementing the trait and
//! registering in [`widget_for`] — the message card never changes.
use eframe::egui;
use crate::data::{Message, MsgKind};

pub mod attachment;
pub mod code;
pub mod schematic;
pub mod text;

pub trait ChatWidget {
    fn show(&self, ui: &mut egui::Ui, msg: &Message);
}

pub fn widget_for(kind: &MsgKind) -> Box<dyn ChatWidget> {
    match kind {
        MsgKind::Question => Box::new(text::QuestionWidget),
        MsgKind::Schematic => Box::new(schematic::SchematicWidget),
        MsgKind::Code => Box::new(code::CodeWidget),
        MsgKind::Attachment => Box::new(attachment::AttachmentWidget),
    }
}

/// Dispatch helper used by the message card.
pub fn render_payload(ui: &mut egui::Ui, msg: &Message) {
    widget_for(&msg.kind).show(ui, msg);
}

/// Shared body renderer for all widgets.
pub fn body(ui: &mut egui::Ui, msg: &Message) {
    ui.label(
        egui::RichText::new(msg.body)
            .size(crate::theme::type_::BODY_MD)
            .color(crate::theme::ON_SURFACE),
    );
}
