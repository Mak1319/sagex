//! SVG image loader (resvg + tiny-skia) so vendored Iconoir `.svg`
//! assets render crisply at any size. No PNGs anywhere.
//!
//! Iconoir glyphs use `stroke="currentColor"`; we rewrite that to white at
//! parse time so `egui::Image::tint(color)` multiplies correctly
//! (white × tint = tint; black × tint would stay black).
use std::{collections::HashMap, sync::Mutex};

use eframe::egui;
use egui::load::{ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};

#[derive(Default)]
pub struct SvgLoader {
    cache: Mutex<HashMap<String, std::sync::Arc<egui::ColorImage>>>,
}

impl SvgLoader {
    pub fn install(ctx: &egui::Context) {
        ctx.add_image_loader(std::sync::Arc::new(Self::default()));
    }

    fn rasterize(bytes: &[u8], width_px: u32, height_px: u32) -> Result<egui::ColorImage, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        // white base so callers can tint to any color
        let text = text.replace("currentColor", "white");
        let opts = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_data(text.as_bytes(), &opts).map_err(|e| e.to_string())?;
        let page = tree.size();
        if page.width() == 0.0 || page.height() == 0.0 {
            return Err("zero-size svg".into());
        }
        let sx = width_px as f32 / page.width();
        let sy = height_px as f32 / page.height();
        let mut pix = resvg::tiny_skia::Pixmap::new(width_px, height_px)
            .ok_or_else(|| "pixmap alloc failed".to_string())?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(sx, sy),
            &mut pix.as_mut(),
        );
        Ok(egui::ColorImage::from_rgba_unmultiplied(
            [width_px as usize, height_px as usize],
            pix.data(),
        ))
    }
}

impl ImageLoader for SvgLoader {
    fn id(&self) -> &str {
        concat!(module_path!(), "::SvgLoader")
    }

    fn load(
        &self,
        ctx: &egui::Context,
        uri: &str,
        size_hint: SizeHint,
    ) -> ImageLoadResult {
        if !uri.ends_with(".svg") {
            return Err(LoadError::NotSupported);
        }
        // size_hint from `fit_to_exact_size` arrives as Size{..} or Width/Height
        let req = match size_hint {
            SizeHint::Width(w) => w.max(1),
            SizeHint::Height(h) => h.max(1),
            SizeHint::Size { width, height, .. } => width.max(1).max(height.max(1)),
            SizeHint::Scale(_) => 48,
        };
        // keep square aspect for our 24x24 icon grid
        let side = req.min(256);
        let key = format!("{uri}@{side}");
        if let Some(hit) = self.cache.lock().unwrap().get(&key) {
            return Ok(ImagePoll::Ready { image: hit.clone() });
        }
        use egui::load::BytesPoll;
        let bytes = match ctx.try_load_bytes(uri) {
            Ok(BytesPoll::Ready { bytes, .. }) => bytes,
            Ok(BytesPoll::Pending { .. }) => return Ok(ImagePoll::Pending { size: None }),
            Err(e) => return Err(LoadError::Loading(format!("{e:?}"))),
        };
        let img = Self::rasterize(&bytes, side, side).map_err(|e| {
            eprintln!("[sagex-varta] svg-loader: {uri} failed: {e}");
            LoadError::Loading(e)
        })?;
        let img = std::sync::Arc::new(img);
        self.cache.lock().unwrap().insert(key, img.clone());
        Ok(ImagePoll::Ready { image: img })
    }

    fn forget(&self, uri: &str) {
        self.cache.lock().unwrap().retain(|k, _| !k.starts_with(uri));
    }

    fn forget_all(&self) {
        self.cache.lock().unwrap().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .unwrap()
            .values()
            .map(|img| img.pixels.len() * 4)
            .sum()
    }
}
