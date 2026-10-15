//! Binary-embedded assets via `rust-embed`.
//!
//! `#[folder = "assets/"]` bakes `assets/icons/*.svg` (and fonts) into the
//! binary so `svg().path("icons/...")` resolves from embedded bytes at
//! runtime instead of requiring an `assets/` dir next to the executable.
//! Unknown paths fall through to the kit default icon bundle.

use std::borrow::Cow;

use gpui_kit::*;

#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
pub struct VartaAssets;

impl AssetSource for VartaAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = Self::get(path) {
            return Ok(Some(file.data));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = Self::iter()
            .filter(|name| name.starts_with(path))
            .map(|name| name.into())
            .collect();
        if let Ok(mut rest) = gpui_kit::assets::Assets.list(path) {
            out.append(&mut rest);
        }
        Ok(out)
    }
}
