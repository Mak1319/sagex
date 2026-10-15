//! Blueprint Precision theme tokens (from DESIGN.md).
//! Plain constants — application presentation layer, no behavior.

pub mod colors {
    pub const SURFACE: u32 = 0xf8f9ff;
    pub const BACKGROUND: u32 = 0xf8f9ff;
    pub const SURFACE_LOWEST: u32 = 0xffffff;
    pub const SURFACE_LOW: u32 = 0xeff4ff;
    pub const SURFACE_CONTAINER: u32 = 0xe5eeff;
    pub const SURFACE_HIGH: u32 = 0xdce9ff;
    pub const SURFACE_HIGHEST: u32 = 0xd3e4fe;
    pub const ON_SURFACE: u32 = 0x0b1c30;
    pub const PRIMARY: u32 = 0x004ac6;
    pub const PRIMARY_DARK: u32 = 0x1d4ed8;
    pub const ON_PRIMARY: u32 = 0xffffff;
    pub const PRIMARY_CONTAINER: u32 = 0x2563eb;
    pub const SECONDARY_CONTAINER: u32 = 0xdae2fd;
    pub const TERTIARY_CONTAINER: u32 = 0x00759f;
    pub const ERROR: u32 = 0xba1a1a;
    pub const GRID_L1: u32 = 0xe2e8f0;
    pub const GRID_L2: u32 = 0xcbd5e1;
    pub const SUCCESS: u32 = 0x059669;
    pub const WARNING: u32 = 0xd97706;
    pub const CRITICAL: u32 = 0xdc2626;
    pub const INK: u32 = 0x0f172a;
    pub const MUTED: u32 = 0x64748b;
    pub const INSET: u32 = 0xf1f5f9;
    /// Row hover — same wash as the icon rail shell.
    pub const ROW_HOVER: u32 = SURFACE_LOW;
    /// Row active — one step deeper than the rail shell.
    pub const ROW_ACTIVE: u32 = SURFACE_CONTAINER;
}

pub mod fonts {
    pub const SANS: &str = "Google Sans";
    pub const HEADLINE: &str = "Space Grotesk";
    pub const BODY: &str = "Geist";
    pub const MONO: &str = "JetBrains Mono";
}

pub mod radius {
    pub const SM: f32 = 2.0;
    pub const DEFAULT: f32 = 4.0;
    pub const LG: f32 = 8.0;
}

/// Convert 0xRRGGBB + alpha into gpui-kit Hsla.
pub fn hsla(hex: u32, alpha: f32) -> gpui_kit::Hsla {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;
    // rgb -> hsl
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let (h, s) = if (max - min).abs() < f32::EPSILON {
        (0.0, 0.0)
    } else {
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
        } else if max == g {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };
        (h, s)
    };
    gpui_kit::hsla(h, s, l, alpha)
}

pub fn rgb(hex: u32) -> gpui_kit::Hsla {
    hsla(hex, 1.0)
}
