//! App SVG icon bundle (Lucide, ISC-licensed, see `assets/icons/`).
//!
//! `gpui-component` icons resolve `icons/<name>.svg` through the GPUI
//! [`AssetSource`](gpui::AssetSource). This crate previously set no asset
//! source, so every SVG-backed glyph (checkbox tick, Sun/Moon, logo,
//! password eye toggle) rendered empty. Wiring [`SvgAssets`] via
//! `Application::with_assets` fixes all of them at once.

use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

macro_rules! icons {
    ($($name:literal),*) => {
        /// SVG file names (without directory) served under `icons/`.
        const ICONS: &[&str] = &[$($name),*];

        fn bytes(name: &str) -> Option<&'static [u8]> {
            match name {
                $($name => Some(include_str!(concat!("../assets/icons/", $name, ".svg")).as_bytes()),)*
                _ => None,
            }
        }
    };
}

icons!(
    "message-circle",
    "phone",
    "circle-dot",
    "users",
    "paperclip",
    "smile",
    "mic",
    "video",
    "send",
    "check",
    "sun",
    "moon",
    "search",
    "ellipsis-vertical",
    "gallery-vertical-end",
    "eye",
    "eye-off",
    "plus",
    "settings",
    "file-text",
    "image",
    "camera",
    "headphones",
    "user",
    "chart-column",
    "calendar",
    "sticker",
    "play",
    "download",
    "chevron-down",
    "tail-in",
    "tail-out",
    "user-plus",
    "info",
    "square-check-big",
    "bell-off",
    "timer",
    "heart",
    "list-plus",
    "x",
    "circle-minus",
    "log-out",
    "pencil",
    "reply",
    "copy",
    "forward",
    "pin",
    "sparkles",
    "star",
    "flag",
    "trash-2",
    "chevron-right",
    "link",
    "check-check",
    "sun-moon",
    "archive"
);

pub struct SvgAssets;

impl AssetSource for SvgAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let name = path.strip_prefix("icons/").unwrap_or(path);
        let name = name.strip_suffix(".svg").unwrap_or(name);
        Ok(bytes(name).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        if path == "icons" || path == "icons/" || path.is_empty() {
            Ok(ICONS
                .iter()
                .map(|n| format!("icons/{n}.svg").into())
                .collect())
        } else {
            Ok(vec![])
        }
    }
}
