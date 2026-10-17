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

/// Rasterize `assets/logo/<src_name>` to square hicolor PNGs.
fn render_logo(src_name: &str) {
    for size in [16u32, 32, 48, 128, 256, 512] {
        let dst_name = format!("icon-{size}.png");
        render(
            &format!("../logo/{src_name}"),
            &dst_name,
            size,
            size,
        );
    }
}

fn rust_string(value: &str) -> String {
    format!("{:?}", value)
}

fn generate_licenses() {
    use serde_json::Value;

    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = base.join("licenses.json");
    println!("cargo:rerun-if-changed=licenses.json");
    let json = std::fs::read_to_string(&source).expect("read licenses.json");
    let document: Value = serde_json::from_str(&json).expect("parse licenses.json");
    let items = document["items"].as_array().expect("licenses.json items array");
    let mut generated = String::from("static PACKAGES: &[Package] = &[\n");
    for item in items {
        let get = |key: &str| item[key].as_str();
        let required = |key: &str| get(key).unwrap_or_else(|| panic!("inventory item missing string field `{key}`"));
        let optional = |key: &str| match get(key) {
            Some(value) => format!("Some({})", rust_string(value)),
            None => "None".to_string(),
        };
        generated.push_str(&format!(
            "    Package {{ name: {}, version: {}, license: {}, author: {}, category: {}, icon: {}, description: {} }},\n",
            rust_string(required("name")), optional("version"), optional("license"),
            optional("author"), rust_string(required("category")), rust_string(required("icon")),
            optional("description"),
        ));
    }
    generated.push_str("];\n");
    generated.push_str(&format!("const INVENTORY_JSON: &str = {};\n", rust_string(&json)));
    let out = Path::new(&std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("licenses_data.rs");
    std::fs::write(out, generated).expect("write generated license inventory");
}

fn main() {
    generate_licenses();
    std::fs::create_dir_all(generated_dir()).expect("create assets/generated");
    // Wordmark 160x66 display → 2x PNG. Native aspect 1207:500 preserved.
    render("sagex-varta.svg", "wordmark.png", 320, 132);
    // Scenery panel is ~400x374 display; the source is square and runtime
    // `ObjectFit::Cover` crops cheaply, so render square at 2x height.
    render("output.svg", "scenery.png", 800, 800);
    // Window/taskbar icon (Linux): GPUI 0.2.2 exposes no `window_icon`
    // API, so the compositor resolves the icon via app_id → .desktop →
    // hicolor theme. Rasterize the square logo once per size here.
    render_logo("varta.svg");
}
