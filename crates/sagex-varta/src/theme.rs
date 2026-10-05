//! DESIGN.md "Blueprint Precision" tokens (light theme).
#![allow(dead_code)]
use eframe::egui;

// surfaces
pub const SURFACE: egui::Color32 = hex(0xF8, 0xF9, 0xFF);
pub const SURFACE_DIM: egui::Color32 = hex(0xCB, 0xDB, 0xF5);
pub const LOWEST: egui::Color32 = egui::Color32::WHITE;
pub const LOW: egui::Color32 = hex(0xEF, 0xF4, 0xFF);
pub const CONTAINER: egui::Color32 = hex(0xE5, 0xEE, 0xFF);
pub const HIGH: egui::Color32 = hex(0xDC, 0xE9, 0xFF);
pub const HIGHEST: egui::Color32 = hex(0xD3, 0xE4, 0xFE);
// ink
pub const ON_SURFACE: egui::Color32 = hex(0x0B, 0x1C, 0x30);
pub const ON_VARIANT: egui::Color32 = hex(0x43, 0x46, 0x55);
pub const OUTLINE: egui::Color32 = hex(0x73, 0x76, 0x86);
pub const OUTLINE_VAR: egui::Color32 = hex(0xC3, 0xC6, 0xD7);
// roles
pub const PRIMARY: egui::Color32 = hex(0x00, 0x4A, 0xC6);
pub const PRIMARY_CTR: egui::Color32 = hex(0x25, 0x63, 0xEB);
pub const ON_PRIMARY_CTR: egui::Color32 = hex(0xEE, 0xEF, 0xFF);
pub const SECONDARY: egui::Color32 = hex(0x56, 0x5E, 0x74);
pub const SECONDARY_CTR: egui::Color32 = hex(0xDA, 0xE2, 0xFD);
pub const TERTIARY_CTR: egui::Color32 = hex(0x00, 0x75, 0x9F);
pub const TERTIARY_BG: egui::Color32 = hex(0xE1, 0xF2, 0xFF);
pub const ERROR: egui::Color32 = hex(0xBA, 0x1A, 0x1A);
pub const ERROR_CTR: egui::Color32 = hex(0xFF, 0xDA, 0xD6);
pub const ERROR_ON_CTR: egui::Color32 = hex(0x93, 0x00, 0x0A);
pub const PRIMARY_FIXED: egui::Color32 = hex(0xDB, 0xE1, 0xFF);
pub const PRIMARY_FIXED_DIM: egui::Color32 = hex(0xB4, 0xC5, 0xFF);
pub const ON_PRIMARY_FIXED: egui::Color32 = hex(0x00, 0x17, 0x4B);
// semantics
pub const SUCCESS: egui::Color32 = hex(0x05, 0x96, 0x69);
pub const SUCCESS_BG: egui::Color32 = hex(0xD1, 0xFA, 0xE5);
pub const WARN: egui::Color32 = hex(0xD9, 0x77, 0x06);
pub const WARN_BG: egui::Color32 = hex(0xFE, 0xF3, 0xC7);
// derived
pub const ACTIVE_ROW: egui::Color32 = hex(0xE8, 0xF0, 0xFE);
pub const AMBER: egui::Color32 = hex(0x92, 0x40, 0x0E);
pub const AMBER_BG: egui::Color32 = hex(0xFF, 0xFB, 0xEB);

const fn hex(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// UI theme mode, toggled from the rail.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
    System,
}

impl ThemeMode {
    pub fn next(self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
            ThemeMode::System => ThemeMode::Light,
        }
    }
}

fn light_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.panel_fill = LOWEST;
    v.window_fill = LOWEST;
    v.extreme_bg_color = LOW;
    v.code_bg_color = LOW;
    v.faint_bg_color = CONTAINER;
    v.widgets.noninteractive.bg_fill = SURFACE;
    v.widgets.inactive.bg_fill = LOW;
    v.widgets.hovered.bg_fill = ACTIVE_ROW;
    v.widgets.active.bg_fill = PRIMARY_FIXED;
    v.selection.bg_fill = PRIMARY_FIXED;
    v.selection.stroke = egui::Stroke::new(1.0, PRIMARY);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, OUTLINE_VAR);
    v
}

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x1E, 0x2A, 0x45);
    v.widgets.active.bg_fill = egui::Color32::from_rgb(0x1E, 0x3A, 0x6E);
    v.selection.bg_fill = egui::Color32::from_rgb(0x1E, 0x3A, 0x6E);
    v.selection.stroke = egui::Stroke::new(1.0, PRIMARY_CTR);
    v
}

// sizes (px, from prototype @1440x900)
// column grid: 50px rail | >=300 panel | 1fr center | >=300 panel
pub const RAIL_W: f32 = 50.0;
pub const LEFT_W: f32 = 300.0;
pub const RIGHT_W: f32 = 300.0;
pub const PANEL_MIN: f32 = 300.0;
pub const DOCK_W: f32 = 36.0;
pub const AVATAR: f32 = 28.0;
pub const ROW_H: f32 = 52.0;

pub mod type_ {
    pub const LABEL_SM: f32 = 10.0;
    pub const LABEL_MD: f32 = 11.0;
    pub const BODY_SM: f32 = 12.0;
    pub const BODY_MD: f32 = 13.0;
    pub const CODE: f32 = 11.0;
    pub const HEAD_SM: f32 = 15.0;
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let resolved = match mode {
        ThemeMode::Light => egui::Theme::Light,
        ThemeMode::Dark => egui::Theme::Dark,
        ThemeMode::System => ctx.system_theme().unwrap_or(egui::Theme::Light),
    };
    let v = match resolved {
        egui::Theme::Light => light_visuals(),
        egui::Theme::Dark => dark_visuals(),
    };
    ctx.set_visuals(v);

    // NOTE: mutate in place — never replace the whole Style, or the
    // light Visuals set above get clobbered by Style::default()'s dark ones.
    ctx.all_styles_mut(|s| {
        s.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(2);
        s.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(2);
        s.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(2);
        s.visuals.widgets.active.corner_radius = egui::CornerRadius::same(2);
        s.spacing.item_spacing = egui::vec2(4.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
    });
}
