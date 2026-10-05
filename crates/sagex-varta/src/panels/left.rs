//! Left conversation index with three manual modes.
//!
//! Grid contract: default 300px, resizable up to 420px. Dragging the edge
//! below 300px collapses to the dock (width readback, explicit state — never
//! `show_collapsible`, whose internal collapsed rendering caused the black
//! strip artifact).
use eframe::egui;
use crate::{data, icons::Icon, theme::*};
use crate::components::{
    node_row::node_row,
    primitives::{chip, kbd, label, section_bar},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LeftMode {
    /// >=300px full index (resizable up to 420).
    Expanded,
    /// 36px slim dock with an expand button.
    Dock,
    /// Fully hidden.
    Hidden,
}

impl LeftMode {
    pub fn next(self) -> Self {
        match self {
            LeftMode::Expanded => LeftMode::Dock,
            LeftMode::Dock => LeftMode::Hidden,
            LeftMode::Hidden => LeftMode::Expanded,
        }
    }
}

pub struct LeftState {
    pub mode: LeftMode,
    pub search: String,
}

impl LeftState {
    pub fn new() -> Self {
        Self { mode: LeftMode::Expanded, search: String::new() }
    }
}

pub fn show(ui: &mut egui::Ui, st: &mut LeftState) {
    match st.mode {
        LeftMode::Hidden => {}
        LeftMode::Dock => {
            egui::Panel::left("left_dock").resizable(false).default_size(DOCK_W).show_separator_line(false).show(
                ui,
                |ui| {
                    ui.vertical_centered(|ui| {
                        if ui.small_button("»").clicked() {
                            st.mode = LeftMode::Expanded;
                        }
                        ui.label(label("NODES", 8.0, PRIMARY));
                        chip(ui, "0x02A", PRIMARY, ACTIVE_ROW);
                    });
                },
            );
        }
        LeftMode::Expanded => {
            let resp = egui::Panel::left("left_index")
                .resizable(true)
                .default_size(LEFT_W)
                .min_size(DOCK_W)
                .max_size(420.0)
                .show_separator_line(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut st.search)
                                .hint_text("Query CAD nodes...")
                                .desired_width(ui.available_width() - 52.0),
                        );
                        kbd(ui, "Ctrl K");
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        section_bar(ui, Icon::Pin, "PINNED NODES (3)", "#P-SYS");
                        for n in data::pinned() {
                            node_row(ui, &n);
                        }
                        section_bar(ui, Icon::Clock, "SCHEMATIC STREAMS", "FEED: LIVE");
                        for n in data::streams() {
                            node_row(ui, &n);
                        }
                    });
                })
                .response;
            // drag-below-min collapses to the dock (hysteresis via the
            // non-resizable dock: no oscillation at the boundary)
            if resp.rect.width() < PANEL_MIN {
                st.mode = LeftMode::Dock;
            }
        }
    }
}
