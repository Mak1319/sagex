//! App composition: rail + left + right + center (+ bottom status).
use eframe::egui;
use crate::{components::composer::Composer, fonts, panels, svg_loader, theme::ThemeMode};

pub struct VartaApp {
    left: panels::left::LeftState,
    right: panels::right::RightState,
    center: panels::center::CenterState,
    theme: ThemeMode,
    fonts_done: bool,
}

impl VartaApp {
    pub fn new() -> Self {
        Self {
            left: panels::left::LeftState::new(),
            right: panels::right::RightState::new(),
            center: panels::center::CenterState { composer: Composer::new() },
            theme: ThemeMode::default(),
            fonts_done: false,
        }
    }
}

impl eframe::App for VartaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::theme::apply(ui.ctx(), self.theme);
        if !self.fonts_done {
            fonts::install(ui.ctx());
            svg_loader::SvgLoader::install(ui.ctx());
            self.fonts_done = true;
        }
        panels::rail::show(ui, &mut self.left.mode, &mut self.theme);
        panels::chrome::bottom(ui);
        panels::left::show(ui, &mut self.left);
        panels::right::show(ui, &mut self.right);
        panels::center::show(ui, &mut self.center);
    }
}
