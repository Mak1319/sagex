use gpui::rgb;

/// Blueprint Precision tokens from `layout/.../DESIGN.md`.
/// Canvas #090A0F, panels #0F121C / #141824, hairlines #1E2436,
/// primary #2563EB, ok #10B981, warn #F59E0B, 4px radii.
pub mod palette {
    use super::*;
    pub fn canvas() -> gpui::Rgba {
        rgb(0x090A0F)
    }
    pub fn panel() -> gpui::Rgba {
        rgb(0x0F121C)
    }
    pub fn panel_hi() -> gpui::Rgba {
        rgb(0x141824)
    }
    pub fn hairline() -> gpui::Rgba {
        rgb(0x1E2436)
    }
    pub fn primary() -> gpui::Rgba {
        rgb(0x2563EB)
    }
    pub fn ok() -> gpui::Rgba {
        rgb(0x10B981)
    }
    pub fn warn() -> gpui::Rgba {
        rgb(0xF59E0B)
    }
    pub fn danger() -> gpui::Rgba {
        rgb(0xF87171)
    }
    pub fn ink() -> gpui::Rgba {
        rgb(0xF8FAFC)
    }
    pub fn ink_dim() -> gpui::Rgba {
        rgb(0x94A3B8)
    }
    pub fn ink_faint() -> gpui::Rgba {
        rgb(0x64748B)
    }
}
