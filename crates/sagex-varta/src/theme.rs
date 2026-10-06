//! Blueprint Precision theme — single source of truth for color.
//! All components read tokens from here via `cx`. No hardcoded colors
//! in component files.

use gpui_kit::{App, Global, Rgba};

fn c(r: u8, g: u8, b: u8) -> Rgba {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// Full DESIGN.md color + shape + spacing tokens.
#[derive(Clone, Copy)]
pub struct BlueprintTheme {
    // surfaces
    pub surface: Rgba,
    pub surface_dim: Rgba,
    pub surface_bright: Rgba,
    pub lowest: Rgba,
    pub low: Rgba,
    pub container: Rgba,
    pub high: Rgba,
    pub highest: Rgba,
    pub background: Rgba,
    pub card: Rgba,
    pub inset: Rgba,
    pub canvas: Rgba,
    // ink
    pub on_surface: Rgba,
    pub on_variant: Rgba,
    pub ink: Rgba,
    pub muted: Rgba,
    pub slate: Rgba,
    // lines
    pub line: Rgba,
    pub line_strong: Rgba,
    pub outline: Rgba,
    // brand
    pub primary: Rgba,
    pub on_primary: Rgba,
    pub primary_container: Rgba,
    pub cobalt: Rgba,
    // semantics
    pub success: Rgba,
    pub warning: Rgba,
    pub critical: Rgba,
    // code + accent hues sourced from the approved reference palette
    // (stitch code.html tailwind scale); kept here so components
    // never hardcode colors.
    pub violet: Rgba,
    pub code_plain: Rgba,
    /// White wash at 40% over the canvas (message card resting fill).
    pub card_wash: Rgba,
    /// White hairline at 30% for dividers on primary fills.
    pub on_primary_dim: Rgba,
    // shape (px)
    pub radius_sm: f32,
    pub radius: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
}

impl Default for BlueprintTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl BlueprintTheme {
    pub fn light() -> Self {
        Self {
            surface: c(0xf8, 0xf9, 0xff),
            surface_dim: c(0xcb, 0xdb, 0xf5),
            surface_bright: c(0xf8, 0xf9, 0xff),
            lowest: c(0xff, 0xff, 0xff),
            low: c(0xef, 0xf4, 0xff),
            container: c(0xe5, 0xee, 0xff),
            high: c(0xdc, 0xe9, 0xff),
            highest: c(0xd3, 0xe4, 0xfe),
            background: c(0xf8, 0xf9, 0xff),
            card: c(0xff, 0xff, 0xff),
            inset: c(0xf1, 0xf5, 0xf9),
            canvas: c(0xf8, 0xfa, 0xfc),
            on_surface: c(0x0b, 0x1c, 0x30),
            on_variant: c(0x43, 0x46, 0x55),
            ink: c(0x0b, 0x1c, 0x30),
            muted: c(0x64, 0x74, 0x8b),
            slate: c(0x0f, 0x17, 0x2a),
            line: c(0xe2, 0xe8, 0xf0),
            line_strong: c(0xcb, 0xd5, 0xe1),
            outline: c(0x73, 0x76, 0x86),
            primary: c(0x00, 0x4a, 0xc6),
            on_primary: c(0xff, 0xff, 0xff),
            primary_container: c(0x25, 0x63, 0xeb),
            cobalt: c(0x1d, 0x4e, 0xd8),
            success: c(0x05, 0x96, 0x69),
            warning: c(0xd9, 0x77, 0x06),
            critical: c(0xdc, 0x26, 0x26),
            violet: c(0x93, 0x33, 0xea),
            code_plain: c(0x1e, 0x29, 0x3b),
            card_wash: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.4,
            },
            on_primary_dim: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.3,
            },
            radius_sm: 2.0,
            radius: 4.0,
            radius_md: 6.0,
            radius_lg: 8.0,
        }
    }
}

impl Global for BlueprintTheme {}

/// Pastel tile tint (avatar chips, role chips): background wash,
/// strong foreground ink, hairline border. Values mirror the
/// reference palette's 50/700/200 steps per hue.
#[derive(Clone, Copy)]
pub struct Tint {
    pub bg: Rgba,
    pub fg: Rgba,
    pub border: Rgba,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TintColor {
    Red,
    Blue,
    Emerald,
    Purple,
    Orange,
    Cyan,
    Slate,
    Indigo,
    Amber,
}

impl BlueprintTheme {
    pub fn tint(&self, color: TintColor) -> Tint {
        let _ = self;
        match color {
            TintColor::Red => Tint {
                bg: c(0xfe, 0xf2, 0xf2),
                fg: c(0xb9, 0x1c, 0x1c),
                border: c(0xfe, 0xca, 0xca),
            },
            TintColor::Blue => Tint {
                bg: c(0xef, 0xf6, 0xff),
                fg: c(0x1d, 0x4e, 0xd8),
                border: c(0xbf, 0xdb, 0xfe),
            },
            TintColor::Emerald => Tint {
                bg: c(0xec, 0xfd, 0xf5),
                fg: c(0x04, 0x78, 0x57),
                border: c(0xa7, 0xf3, 0xd0),
            },
            TintColor::Purple => Tint {
                bg: c(0xfa, 0xf5, 0xff),
                fg: c(0x7e, 0x22, 0xce),
                border: c(0xe9, 0xd5, 0xff),
            },
            TintColor::Orange => Tint {
                bg: c(0xff, 0xf7, 0xed),
                fg: c(0xc2, 0x41, 0x0c),
                border: c(0xfe, 0xd7, 0xaa),
            },
            TintColor::Cyan => Tint {
                bg: c(0xec, 0xfe, 0xff),
                fg: c(0x0e, 0x74, 0x90),
                border: c(0xa5, 0xf3, 0xfc),
            },
            TintColor::Slate => Tint {
                bg: c(0xf1, 0xf5, 0xf9),
                fg: c(0x33, 0x41, 0x55),
                border: c(0xe2, 0xe8, 0xf0),
            },
            TintColor::Indigo => Tint {
                bg: c(0xee, 0xf2, 0xff),
                fg: c(0x43, 0x38, 0xca),
                border: c(0xc7, 0xd2, 0xfe),
            },
            TintColor::Amber => Tint {
                bg: c(0xff, 0xfb, 0xeb),
                fg: c(0x78, 0x35, 0x0e),
                border: c(0xfc, 0xd3, 0x4d),
            },
        }
    }
}

/// Install the theme into the app context. Call once after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    cx.set_global(BlueprintTheme::light());
}

/// Read the active theme from context.
pub fn theme(cx: &App) -> BlueprintTheme {
    *cx.global::<BlueprintTheme>()
}
