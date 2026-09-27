//! Runtime emoji font registration.
//!
//! GPUI's built-in fallback stack has no emoji entry, so color emoji can
//! render as tofu even when the OS ships a font. This module probes well-
//! known system emoji fonts and registers the first one found via
//! `TextSystem::add_fonts`. No font is vendored (Noto Color Emoji is ~10MB).
//!
//! Other machines need any single system emoji font installed.

use gpui::App;
use std::borrow::Cow;

const CANDIDATES: &[&str] = &[
    // Linux
    "/usr/share/fonts/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/TwemojiMozilla.ttf",
    "/usr/share/fonts/truetype/twemoji/TwemojiMozilla.ttf",
    // macOS
    "/System/Library/Fonts/Apple Color Emoji.ttc",
    // Windows
    "C:\\Windows\\Fonts\\seguiemj.ttf",
];

/// Try each candidate path; register the first hit. Returns the path used.
pub fn register_emoji_font(cx: &mut App) -> Option<&'static str> {
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            let bytes: Cow<'static, [u8]> = Cow::Owned(bytes);
            if cx.text_system().add_fonts(vec![bytes]).is_ok() {
                println!("emoji font registered: {path}");
                return Some(*path);
            }
        }
    }
    println!("emoji font: none found, install Noto Color Emoji (or OS equivalent)");
    None
}
