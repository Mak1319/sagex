use gpui_kit::*;
use std::borrow::Cow;

/// Local embedded assets from `crates/sagex-varta/assets/`.
/// Falls back to the kit catalog so stock icons keep working.
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
#[include = "icons/*.svg"]
#[include = "fonts/*.ttf"]
#[include = "logo/*.svg"]
pub struct LocalAssets;

impl AssetSource for LocalAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(f) = Self::get(path) {
            return Ok(Some(f.data));
        }
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = Self::iter()
            .filter(|n| n.starts_with(path))
            .map(|n| n.into())
            .collect();
        if let Ok(mut rest) = gpui_kit::assets::AllAssets.list(path) {
            out.append(&mut rest);
        }
        Ok(out)
    }
}
