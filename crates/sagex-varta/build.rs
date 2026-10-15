//! Build-time SVG → PNG rasterization for the splash screen.
//!
//! Runtime `img()` on the 4MB `output.svg` re-rasterizes dozens of complex
//! vector paths every launch. Pre-rendering once here at 2x (the same scale
//! GPUI uses for SVGs) reduces startup to a cheap PNG decode. Outputs go to
//! `assets/generated/` so the existing rust-embed `#[folder = "assets/"]`
//! picks them up with no `assets.rs` changes. Re-renders only when a source
//! SVG is newer than its PNG.

use std::path::{Path, PathBuf};

fn generated_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/generated")
}

fn up_to_date(src: &Path, dst: &Path) -> bool {
    match (src.metadata(), dst.metadata()) {
        (Ok(s), Ok(d)) => match (s.modified(), d.modified()) {
            (Ok(ss), Ok(dd)) => dd >= ss,
            _ => false,
        },
        _ => false,
    }
}

fn render(src_name: &str, dst_name: &str, target_w: u32, target_h: u32) {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = base.join("assets/images").join(src_name);
    let dst = generated_dir().join(dst_name);
    println!("cargo:rerun-if-changed=assets/images/{}", src_name);
    if up_to_date(&src, &dst) {
        return;
    }
    let data = std::fs::read(&src).expect("read splash svg");
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())
        .expect("parse splash svg");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_w, target_h).expect("png canvas");
    let sx = target_w as f32 / tree.size().width();
    let sy = target_h as f32 / tree.size().height();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(&dst).expect("write splash png");
}

fn main() {
    std::fs::create_dir_all(generated_dir()).expect("create assets/generated");
    // Wordmark 160x66 display → 2x PNG. Native aspect 1207:500 preserved.
    render("sagex-varta.svg", "wordmark.png", 320, 132);
    // Scenery panel is ~400x374 display; the source is square and runtime
    // `ObjectFit::Cover` crops cheaply, so render square at 2x height.
    render("output.svg", "scenery.png", 800, 800);
}
