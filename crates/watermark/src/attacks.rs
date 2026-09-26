//! Test-attack simulation: rotation, scaling, cropping, lighting, JPEG.
//!
//! WHY this module exists: watermark robustness cannot be judged on clean
//! images. Real channels rotate/scale (printing, screenshots), reframe
//! (photo borders, crops), relight (screen brightness, room light gradients)
//! and re-encode (social media JPEG). `--test-attack
//! "rotate:5,scale:0.9,crop:0.05,bright:1.1,jpeg:80"` applies those
//! in-memory so you can measure bit-error-rate (BER) without external
//! tools. Attacks run in RGB space (like the real world), while the
//! watermark lives in Y — so Cb/Cr damage does not flatter the score.

use image::{Rgb, RgbImage};

/// Parsed attack chain. Each field `None` means "skip".
#[derive(Debug, Clone, Default)]
pub struct AttackSpec {
    /// Degrees counter-clockwise.
    pub rotate_deg: Option<f32>,
    /// e.g. 0.9 = shrink 10% then center-crop/pad back to original size.
    pub scale: Option<f32>,
    /// Fraction per side to center-crop then resize back (0..0.4).
    /// Simulates photo framing error + resampling.
    pub crop: Option<f32>,
    /// Fraction of rows blacked out from the bottom (0..0.5), no resampling.
    /// Models occlusions/borders: content stays aligned (burst erasure),
    /// which tiling + majority vote is designed to survive.
    pub occlude: Option<f32>,
    /// Brightness gain (1.0 = identity). Simulates screen/camera exposure.
    pub bright: Option<f32>,
    /// JPEG quality 1..100.
    pub jpeg_quality: Option<u8>,
}

/// Parse `"rotate:5,scale:0.9,crop:0.05,bright:1.1,jpeg:80"`
/// (keys also accept `rot`, `s`, `jpg`, `q`, `b`, `c`).
pub fn parse(spec: &str) -> Result<AttackSpec, String> {
    let mut out = AttackSpec::default();
    let s = spec.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        return Ok(out);
    }
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (k, v) = part
            .split_once(':')
            .ok_or_else(|| format!("bad attack item '{part}' (want key:value)"))?;
        match k.trim().to_lowercase().as_str() {
            "rotate" | "rot" | "r" => {
                out.rotate_deg = Some(v.trim().parse::<f32>().map_err(|e| e.to_string())?);
            }
            "scale" | "s" => {
                let f = v.trim().parse::<f32>().map_err(|e| e.to_string())?;
                if !(0.2..=5.0).contains(&f) {
                    return Err(format!("scale {f} out of range 0.2..5.0"));
                }
                out.scale = Some(f);
            }
            "jpeg" | "jpg" | "jpeg_quality" | "q" => {
                let q = v.trim().parse::<u8>().map_err(|e| e.to_string())?;
                if q == 0 {
                    return Err("jpeg quality must be 1..100".into());
                }
                out.jpeg_quality = Some(q.min(100));
            }
            "crop" | "c" => {
                let f = v.trim().parse::<f32>().map_err(|e| e.to_string())?;
                if !(0.0..=0.4).contains(&f) {
                    return Err(format!("crop {f} out of range 0..0.4"));
                }
                out.crop = Some(f);
            }
            "occlude" | "occ" | "o" => {
                let f = v.trim().parse::<f32>().map_err(|e| e.to_string())?;
                if !(0.0..=0.5).contains(&f) {
                    return Err(format!("occlude {f} out of range 0..0.5"));
                }
                out.occlude = Some(f);
            }
            "bright" | "brightness" | "b" => {
                let f = v.trim().parse::<f32>().map_err(|e| e.to_string())?;
                if !(0.2..=5.0).contains(&f) {
                    return Err(format!("bright {f} out of range 0.2..5.0"));
                }
                out.bright = Some(f);
            }
            other => return Err(format!("unknown attack '{other}'")),
        }
    }
    Ok(out)
}

/// Apply the full chain in order: rotate -> scale -> crop -> occlude -> bright -> jpeg.
pub fn apply_all(img: &RgbImage, spec: &AttackSpec) -> RgbImage {
    let mut cur = img.clone();
    if let Some(deg) = spec.rotate_deg {
        cur = apply_rotate(&cur, deg);
    }
    if let Some(f) = spec.scale {
        cur = apply_scale(&cur, f);
    }
    if let Some(f) = spec.crop {
        cur = apply_crop_resize(&cur, f);
    }
    if let Some(f) = spec.occlude {
        cur = apply_occlude(&cur, f);
    }
    if let Some(g) = spec.bright {
        cur = apply_brightness(&cur, g);
    }
    if let Some(q) = spec.jpeg_quality {
        cur = apply_jpeg(&cur, q);
    }
    cur
}

/// Rotate about the center, same output size, black fill outside.
/// Bilinear sampling; inverse-mapped so no holes.
pub fn apply_rotate(img: &RgbImage, deg: f32) -> RgbImage {
    let (w, h) = img.dimensions();
    let mut out = RgbImage::new(w, h);
    let rad = deg.to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            // Output -> source (inverse rotation).
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let sx = cos * dx + sin * dy + cx;
            let sy = -sin * dx + cos * dy + cy;
            out.put_pixel(x, y, sample_bilinear(img, sx, sy));
        }
    }
    out
}

fn sample_bilinear(img: &RgbImage, x: f32, y: f32) -> Rgb<u8> {
    let (w, h) = img.dimensions();
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let at = |xx: i32, yy: i32| -> [f32; 3] {
        if xx < 0 || yy < 0 || xx >= w as i32 || yy >= h as i32 {
            [0.0, 0.0, 0.0]
        } else {
            let p = img.get_pixel(xx as u32, yy as u32).0;
            [p[0] as f32, p[1] as f32, p[2] as f32]
        }
    };
    let a = at(x0, y0);
    let b = at(x0 + 1, y0);
    let c = at(x0, y0 + 1);
    let d = at(x0 + 1, y0 + 1);
    let mix = |i: usize| {
        (a[i] * (1.0 - fx) * (1.0 - fy)
            + b[i] * fx * (1.0 - fy)
            + c[i] * (1.0 - fx) * fy
            + d[i] * fx * fy)
            .clamp(0.0, 255.0) as u8
    };
    Rgb([mix(0), mix(1), mix(2)])
}

/// Scale then restore original canvas: shrink/grow via Lanczos3, then
/// center-crop (if bigger) or center-pad black (if smaller).
/// Keeps dimensions stable so the extractor needs no size hint.
pub fn apply_scale(img: &RgbImage, factor: f32) -> RgbImage {
    use image::imageops::FilterType;
    let (w, h) = img.dimensions();
    let nw = ((w as f32 * factor).round() as u32).max(1);
    let nh = ((h as f32 * factor).round() as u32).max(1);
    let resized = image::imageops::resize(img, nw, nh, FilterType::Lanczos3);
    if nw == w && nh == h {
        return resized;
    }
    let mut out = RgbImage::from_pixel(w, h, Rgb([0, 0, 0]));
    // Overlay centered.
    let ox = (w as i32 - nw as i32) / 2;
    let oy = (h as i32 - nh as i32) / 2;
    for y in 0..nh {
        for x in 0..nw {
            let dx = x as i32 + ox;
            let dy = y as i32 + oy;
            if dx >= 0 && dy >= 0 && dx < w as i32 && dy < h as i32 {
                out.put_pixel(dx as u32, dy as u32, *resized.get_pixel(x, y));
            }
        }
    }
    out
}

/// JPEG encode/decode round-trip at `quality`. This is where high-frequency
/// DCT coefficients die — the main threat to the watermark.
pub fn apply_jpeg(img: &RgbImage, quality: u8) -> RgbImage {
    use image::codecs::jpeg::JpegEncoder;
    let mut buf = Vec::new();
    {
        let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
        enc.encode_image(img).expect("jpeg encode");
    }
    let dynimg = image::load_from_memory(&buf).expect("jpeg decode");
    dynimg.to_rgb8()
}

/// Center-crop `margin` fraction per side, then resize back to the original
/// canvas. Models photo framing error: border blocks are lost entirely
/// (burst erasure) while survivors are resampled.
pub fn apply_crop_resize(img: &RgbImage, margin: f32) -> RgbImage {
    use image::imageops::FilterType;
    let (w, h) = img.dimensions();
    if margin <= 0.0 {
        return img.clone();
    }
    let mx = (w as f32 * margin) as u32;
    let my = (h as f32 * margin) as u32;
    let cw = w.saturating_sub(2 * mx).max(8);
    let ch = h.saturating_sub(2 * my).max(8);
    let cropped = image::imageops::crop_imm(img, mx, my, cw, ch).to_image();
    image::imageops::resize(&cropped, w, h, FilterType::Lanczos3)
}

/// Global exposure change: out = in * gain (clamped). Models screen
/// brightness / camera auto-exposure. QIM in mid-band AC largely survives
/// pure gain, but clipping at 0/255 destroys coefficients — keep gain sane.
pub fn apply_brightness(img: &RgbImage, gain: f32) -> RgbImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        for ch in p.0.iter_mut() {
            *ch = (*ch as f32 * gain).clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Black out the bottom `frac` of rows (dims preserved, no resampling).
/// Pure burst erasure with content otherwise aligned.
pub fn apply_occlude(img: &RgbImage, frac: f32) -> RgbImage {
    let (w, h) = img.dimensions();
    let mut out = img.clone();
    let cut = (h as f32 * (1.0 - frac)) as u32;
    for y in cut..h {
        for x in 0..w {
            out.put_pixel(x, y, Rgb([0, 0, 0]));
        }
    }
    out
}

/// Bit-error-rate between two bit-slices. Returns (errors, rate).
pub fn ber_bits(a: &[u8], b: &[u8]) -> (usize, f32) {
    assert_eq!(a.len(), b.len(), "BER needs equal lengths");
    if a.is_empty() {
        return (0, 0.0);
    }
    let e = a.iter().zip(b.iter()).filter(|(x, y)| x != y).count();
    (e, e as f32 / a.len() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok() {
        let s = parse("rotate:5,scale:0.9,crop:0.05,occlude:0.2,bright:1.1,jpeg:80").unwrap();
        assert_eq!(s.rotate_deg, Some(5.0));
        assert_eq!(s.scale, Some(0.9));
        assert_eq!(s.crop, Some(0.05));
        assert_eq!(s.occlude, Some(0.2));
        assert_eq!(s.bright, Some(1.1));
        assert_eq!(s.jpeg_quality, Some(80));
        assert!(parse("none").unwrap().rotate_deg.is_none());
    }

    #[test]
    fn identity_attacks_preserve_size() {
        let img = RgbImage::from_fn(32, 24, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
        let r = apply_rotate(&img, 0.0);
        assert_eq!(r.dimensions(), (32, 24));
        let s = apply_scale(&img, 1.0);
        assert_eq!(s.dimensions(), (32, 24));
        let j = apply_jpeg(&img, 95);
        assert_eq!(j.dimensions(), (32, 24));
        let c = apply_crop_resize(&img, 0.1);
        assert_eq!(c.dimensions(), (32, 24));
        let o = apply_occlude(&img, 0.25);
        assert_eq!(o.dimensions(), (32, 24));
        assert_eq!(o.get_pixel(0, 23), &Rgb([0, 0, 0]));
        let b = apply_brightness(&img, 1.0);
        assert_eq!(b.dimensions(), (32, 24));
        assert_eq!(b.get_pixel(3, 4), img.get_pixel(3, 4));
    }

    #[test]
    fn ber_counts() {
        assert_eq!(ber_bits(&[0, 1, 1], &[0, 0, 1]), (1, 1.0 / 3.0));
    }
}
