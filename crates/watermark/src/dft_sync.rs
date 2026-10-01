//! DFT synchronization template + log-polar RST resynchronization.
//!
//! WHY a DFT template at all: QIM payload extraction assumes pixel-accurate
//! alignment. A 2° rotation or 5% scale desynchronizes every 8x8 block and
//! kills the watermark. DFT *magnitude* is translation-invariant, and a
//! rotation/scale of the image becomes the *same* rotation/scale of the
//! magnitude spectrum. By planting an artificial ring of peaks at a known
//! radius (far from the natural 1/f spectral hump near DC), we create a
//! landmark that survives JPEG/low-pass. WHY log-polar: in (log r, θ)
//! coordinates rotation becomes a vertical shift and scaling a horizontal
//! shift, so RST estimation reduces to finding a 2D shift via phase
//! correlation (FFT-normalized cross-correlation, robust to brightness).
//!
//! SIMPLIFICATIONS (documented honestly): two fixed rings (0.25 / 0.40 of
//! Nyquist), 16 peaks each; detector is best-effort — if the correlation
//! peak is weak (heavy attack / small image) we return identity (0°, 1.0x)
//! rather than hallucinate a warp. WHY two rings: JPEG quantization kills
//! high frequencies first, so the outer ring may die while the inner
//! survives (and vice versa under downscaling, which compresses the
//! spectrum inward). Production would use keyed pseudo-random peak phases,
//! multi-ring scale search, and subpixel peak fitting.

use ndarray::Array2;
use num_complex::Complex;
use rustfft::FftPlanner;

/// Radii of the sync rings as fractions of min(H,W)/2.
pub const RING_RATIOS: [f32; 2] = [0.25, 0.40];
/// Legacy single-ring ratio (kept for reference).
#[allow(dead_code)]
pub const RING_RATIO: f32 = RING_RATIOS[0];
/// Number of peaks evenly spaced on the ring.
pub const NUM_PEAKS: usize = 16;
/// Default embed strength (added to complex spectrum, real part).
/// Calibrated for 0-255 Y on 128px+ images: mid-band DFT magnitudes are
/// typically 1e3-1e4, so 4000 plants clearly-above-background peaks while
/// spreading to only ~±2 gray levels spatially (PSNR stays >45 dB).
pub const DEFAULT_STRENGTH: f32 = 4000.0;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Detection {
    /// Estimated rotation in degrees (counter-clockwise positive).
    pub theta_deg: f32,
    /// Estimated scale factor applied to the image.
    pub scale: f32,
    /// Phase-correlation peak height in [0,1]; <0.08 ≈ unreliable.
    pub confidence: f32,
}

// ---------- 2D FFT helpers (row-column decomposition) ----------

fn fft_rows(data: &mut [Complex<f32>], h: usize, w: usize, forward: bool) {
    let mut planner = FftPlanner::<f32>::new();
    let fft = if forward {
        planner.plan_fft_forward(w)
    } else {
        planner.plan_fft_inverse(w)
    };
    let mut buf = vec![Complex::new(0.0, 0.0); w];
    for r in 0..h {
        for c in 0..w {
            buf[c] = data[r * w + c];
        }
        fft.process(&mut buf);
        for c in 0..w {
            data[r * w + c] = buf[c];
        }
    }
}

fn fft_cols(data: &mut [Complex<f32>], h: usize, w: usize, forward: bool) {
    let mut planner = FftPlanner::<f32>::new();
    let fft = if forward {
        planner.plan_fft_forward(h)
    } else {
        planner.plan_fft_inverse(h)
    };
    let mut buf = vec![Complex::new(0.0, 0.0); h];
    for c in 0..w {
        for r in 0..h {
            buf[r] = data[r * w + c];
        }
        fft.process(&mut buf);
        for r in 0..h {
            data[r * w + c] = buf[r];
        }
    }
}

fn forward_2d(data: &mut [Complex<f32>], h: usize, w: usize) {
    fft_rows(data, h, w, true);
    fft_cols(data, h, w, true);
}

fn inverse_2d(data: &mut [Complex<f32>], h: usize, w: usize) {
    fft_cols(data, h, w, false);
    fft_rows(data, h, w, false);
    let n = (h * w) as f32;
    for v in data.iter_mut() {
        *v /= n;
    }
}

fn fftshift_inplace(data: &mut [Complex<f32>], h: usize, w: usize) {
    let mut out = vec![Complex::new(0.0, 0.0); h * w];
    for r in 0..h {
        for c in 0..w {
            let sr = (r + h / 2) % h;
            let sc = (c + w / 2) % w;
            out[sr * w + sc] = data[r * w + c];
        }
    }
    data.copy_from_slice(&out);
}

/// Per-peak strength weight by ring (inner full, outer half).
fn ring_weights(h: usize, w: usize) -> Vec<((usize, usize), f32)> {
    let cy = h as f32 / 2.0;
    let cx = w as f32 / 2.0;
    let mut pts = Vec::with_capacity(NUM_PEAKS * 4);
    for (ri, &ratio) in RING_RATIOS.iter().enumerate() {
        let wt = if ri == 0 { 1.0 } else { 0.5 };
        let radius = (h.min(w) as f32 / 2.0) * ratio * 2.0;
        for k in 0..NUM_PEAKS {
            let theta = 2.0 * std::f32::consts::PI * k as f32 / NUM_PEAKS as f32;
            for (sy, sx) in [(1.0, 1.0), (-1.0, -1.0)] {
                let y = (cy + sy * radius * theta.sin()).round() as isize;
                let x = (cx + sx * radius * theta.cos()).round() as isize;
                if y >= 0 && y < h as isize && x >= 0 && x < w as isize {
                    pts.push(((y as usize, x as usize), wt));
                }
            }
        }
    }
    pts.sort_by_key(|x| x.0);
    pts.dedup_by(|a, b| a.0 == b.0);
    pts
}

/// Embed the sync rings into the Y channel (in place).
///
/// Computes DFT(Y), adds `strength` to the real part at ring bins
/// (outer ring at half weight, plus mirrors), inverse DFTs. The spatial
/// distortion is spread globally like faint noise — far less visible
/// than a spatial watermark.
pub fn embed_sync(y: &mut Array2<f32>, strength: f32) {
    let (h, w) = y.dim();
    if h < 32 || w < 32 {
        return; // too small for a meaningful ring; skip silently.
    }
    let mut spec: Vec<Complex<f32>> = y.iter().map(|&v| Complex::new(v, 0.0)).collect();
    forward_2d(&mut spec, h, w);
    fftshift_inplace(&mut spec, h, w);
    for ((r, c), wt) in ring_weights(h, w) {
        spec[r * w + c].re += strength * wt;
    }
    fftshift_inplace(&mut spec, h, w); // shift is its own inverse for even dims
    inverse_2d(&mut spec, h, w);
    for (dst, s) in y.iter_mut().zip(spec.iter()) {
        *dst = s.re;
    }
}

/// Centered log1p magnitude spectrum, used by the detector.
pub fn magnitude_shifted(y: &Array2<f32>) -> Vec<f32> {
    let (h, w) = y.dim();
    let mut spec: Vec<Complex<f32>> = y.iter().map(|&v| Complex::new(v, 0.0)).collect();
    forward_2d(&mut spec, h, w);
    fftshift_inplace(&mut spec, h, w);
    spec.iter().map(|c| (1.0 + c.norm()).ln()).collect()
}

/// Log-polar resampling of a centered magnitude image.
/// Output is (out_r rows = log-radius) x (out_c cols = angle).
pub fn log_polar(src: &[f32], h: usize, w: usize, out_r: usize, out_c: usize) -> Vec<f32> {
    let cy = h as f32 / 2.0;
    let cx = w as f32 / 2.0;
    let r_min: f32 = 4.0;
    let r_max: f32 = (h.min(w) as f32 / 2.0) - 1.0;
    let log_ratio = (r_max / r_min).ln();
    let mut out = vec![0.0f32; out_r * out_c];
    for or in 0..out_r {
        let rho = or as f32 / out_r as f32;
        let r = r_min * (log_ratio * rho).exp();
        for oc in 0..out_c {
            let theta = 2.0 * std::f32::consts::PI * oc as f32 / out_c as f32;
            let sy = cy + r * theta.sin();
            let sx = cx + r * theta.cos();
            out[or * out_c + oc] = bilinear(src, h, w, sy, sx);
        }
    }
    // Zero-mean helps phase correlation ignore flat background.
    let mean = out.iter().sum::<f32>() / out.len() as f32;
    for v in out.iter_mut() {
        *v -= mean;
    }
    out
}

fn bilinear(src: &[f32], h: usize, w: usize, y: f32, x: f32) -> f32 {
    let y0 = y.floor() as isize;
    let x0 = x.floor() as isize;
    let fy = y - y.floor();
    let fx = x - x.floor();
    let at = |r: isize, c: isize| -> f32 {
        if r < 0 || c < 0 || r >= h as isize || c >= w as isize {
            0.0
        } else {
            src[r as usize * w + c as usize]
        }
    };
    at(y0, x0) * (1.0 - fy) * (1.0 - fx)
        + at(y0, x0 + 1) * (1.0 - fy) * fx
        + at(y0 + 1, x0) * fy * (1.0 - fx)
        + at(y0 + 1, x0 + 1) * fy * fx
}

/// Ideal reference log-polar image: vertical lines where the rings map to
/// (inner full weight, outer half weight to match embedding).
fn ideal_reference(out_r: usize, out_c: usize, h: usize, w: usize) -> Vec<f32> {
    let r_min: f32 = 4.0;
    let r_max: f32 = (h.min(w) as f32 / 2.0) - 1.0;
    let mut out = vec![0.0f32; out_r * out_c];
    for (ri, &ratio) in RING_RATIOS.iter().enumerate() {
        let wt = if ri == 0 { 1.0 } else { 0.5 };
        let ring_r = (h.min(w) as f32 / 2.0) * ratio * 2.0;
        let rho_ring = (ring_r / r_min).ln() / (r_max / r_min).ln();
        let center = (rho_ring * out_r as f32).round() as isize;
        for oc in 0..out_c {
            for dr in -1..=1 {
                let r = center + dr;
                if r >= 0 && r < out_r as isize {
                    out[r as usize * out_c + oc] += wt;
                }
            }
        }
    }
    let mean = out.iter().sum::<f32>() / out.len() as f32;
    for v in out.iter_mut() {
        *v -= mean;
    }
    out
}

/// Phase correlation of two same-size real images; returns (shift_r, shift_c, peak).
fn phase_correlation(a: &[f32], b: &[f32], h: usize, w: usize) -> (isize, isize, f32) {
    let mut fa: Vec<Complex<f32>> = a.iter().map(|&v| Complex::new(v, 0.0)).collect();
    let mut fb: Vec<Complex<f32>> = b.iter().map(|&v| Complex::new(v, 0.0)).collect();
    forward_2d(&mut fa, h, w);
    forward_2d(&mut fb, h, w);
    let mut cross: Vec<Complex<f32>> = fa
        .iter()
        .zip(fb.iter())
        .map(|(x, y)| {
            let c = x * y.conj();
            let m = c.norm();
            if m < 1e-9 {
                Complex::new(0.0, 0.0)
            } else {
                c / m
            }
        })
        .collect();
    inverse_2d(&mut cross, h, w);
    let (mut bi, mut bv) = (0usize, f32::MIN);
    for (i, c) in cross.iter().enumerate() {
        if c.re > bv {
            bv = c.re;
            bi = i;
        }
    }
    let br = (bi / w) as isize;
    let bc = (bi % w) as isize;
    // Wrap into centered shifts.
    let sr = if br > h as isize / 2 {
        br - h as isize
    } else {
        br
    };
    let sc = if bc > w as isize / 2 {
        bc - w as isize
    } else {
        bc
    };
    (sr, sc, bv)
}

/// Detect rotation/scale by correlating the observed log-polar spectrum
/// against the ideal ring template.
pub fn detect(y: &Array2<f32>) -> Detection {
    let (h, w) = y.dim();
    const OR: usize = 64;
    const OC: usize = 128;
    if h < 64 || w < 64 {
        return Detection {
            theta_deg: 0.0,
            scale: 1.0,
            confidence: 0.0,
        };
    }
    let mag = magnitude_shifted(y);
    let lp_obs = log_polar(&mag, h, w, OR, OC);
    let lp_ref = ideal_reference(OR, OC, h, w);
    let (sr, sc, peak) = phase_correlation(&lp_obs, &lp_ref, OR, OC);
    // sc: shift along angle axis -> rotation; sr: along log-radius -> scale.
    let theta = -(sc as f32) / OC as f32 * 360.0;
    let r_min: f32 = 4.0;
    let r_max: f32 = (h.min(w) as f32 / 2.0) - 1.0;
    let scale = ((r_max / r_min).ln() * (-(sr as f32)) / OR as f32).exp();
    if peak < 0.08 {
        // Unreliable — refuse to warp rather than damage the image.
        Detection {
            theta_deg: 0.0,
            scale: 1.0,
            confidence: peak,
        }
    } else {
        Detection {
            theta_deg: theta,
            scale,
            confidence: peak,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn embed_changes_image_slightly() {
        let mut y = Array2::from_shape_fn((64, 64), |(r, c)| (r + c) as f32);
        let before = y.clone();
        embed_sync(&mut y, DEFAULT_STRENGTH);
        let diff: f32 = y
            .iter()
            .zip(before.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / y.len() as f32;
        assert!(diff > 1e-6 && diff < 5.0, "mean abs diff {diff}");
    }

    #[test]
    fn log_polar_shape_and_detect_runs() {
        let y = Array2::from_shape_fn((64, 64), |(r, c)| ((r * c) % 251) as f32);
        let d = detect(&y);
        assert!(d.scale.is_finite() && d.theta_deg.is_finite());
    }
}
