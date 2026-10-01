//! Haar DWT forward/inverse (single level, 2D, separable).
//!
//! WHY Haar: it is the simplest orthonormal wavelet — pairwise
//! mean/difference scaled by 1/sqrt(2). That makes the transform
//! energy-preserving and trivially invertible, ideal for teaching.
//! WHY LL subband for watermarking (see `main.rs`): LL holds most image
//! energy / coarse structure, so it survives JPEG quantization and
//! low-pass filtering far better than LH/HL/HH (edges/details), which
//! are the first casualties of compression. The trade-off is slightly
//! higher visibility, compensated by embedding in mid-frequency DCT
//! coefficients with a tunable QIM step.

use ndarray::Array2;

const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Forward single-level 2D Haar DWT.
///
/// Input must have even height and width (caller pads to multiple of 16,
/// so this always holds). Returns `(LL, LH, HL, HH)` each `(h/2, w/2)`.
///
/// Row pass splits each row into low/high halves; column pass splits
/// each half-column again, yielding the four subbands.
pub fn forward(y: &Array2<f32>) -> (Array2<f32>, Array2<f32>, Array2<f32>, Array2<f32>) {
    let (h, w) = (y.nrows(), y.ncols());
    assert!(h % 2 == 0 && w % 2 == 0, "DWT requires even dimensions");
    let (hh, hw) = (h / 2, w / 2);

    // Row transform: tmp[r][c] for c<hw = low, c>=hw = high.
    let mut tmp = Array2::<f32>::zeros((h, w));
    for r in 0..h {
        for c in 0..hw {
            let a = y[[r, 2 * c]];
            let b = y[[r, 2 * c + 1]];
            tmp[[r, c]] = (a + b) * INV_SQRT2;
            tmp[[r, c + hw]] = (a - b) * INV_SQRT2;
        }
    }

    let mut ll = Array2::<f32>::zeros((hh, hw));
    let mut lh = Array2::<f32>::zeros((hh, hw));
    let mut hl = Array2::<f32>::zeros((hh, hw));
    let mut hh_band = Array2::<f32>::zeros((hh, hw));
    for c in 0..w {
        let out_low = c < hw;
        let oc = if out_low { c } else { c - hw };
        for r in 0..hh {
            let a = tmp[[2 * r, c]];
            let b = tmp[[2 * r + 1, c]];
            let low = (a + b) * INV_SQRT2;
            let high = (a - b) * INV_SQRT2;
            if out_low {
                ll[[r, oc]] = low;
                lh[[r, oc]] = high;
            } else {
                hl[[r, oc]] = low;
                hh_band[[r, oc]] = high;
            }
        }
    }
    (ll, lh, hl, hh_band)
}

/// Inverse single-level 2D Haar DWT. Exact inverse of [`forward`]
/// (up to float rounding) because the basis is orthonormal.
pub fn inverse(
    ll: &Array2<f32>,
    lh: &Array2<f32>,
    hl: &Array2<f32>,
    hh_band: &Array2<f32>,
) -> Array2<f32> {
    let (hh, hw) = (ll.nrows(), ll.ncols());
    let (h, w) = (hh * 2, hw * 2);

    // Invert column pass -> tmp.
    let mut tmp = Array2::<f32>::zeros((h, w));
    for c in 0..hw {
        for r in 0..hh {
            // Left half columns (from LL/LH).
            let l = ll[[r, c]];
            let h1 = lh[[r, c]];
            tmp[[2 * r, c]] = (l + h1) * INV_SQRT2;
            tmp[[2 * r + 1, c]] = (l - h1) * INV_SQRT2;
            // Right half columns (from HL/HH).
            let l2 = hl[[r, c]];
            let h2 = hh_band[[r, c]];
            tmp[[2 * r, c + hw]] = (l2 + h2) * INV_SQRT2;
            tmp[[2 * r + 1, c + hw]] = (l2 - h2) * INV_SQRT2;
        }
    }

    // Invert row pass.
    let mut y = Array2::<f32>::zeros((h, w));
    for r in 0..h {
        for c in 0..hw {
            let l = tmp[[r, c]];
            let h1 = tmp[[r, c + hw]];
            y[[r, 2 * c]] = (l + h1) * INV_SQRT2;
            y[[r, 2 * c + 1]] = (l - h1) * INV_SQRT2;
        }
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    fn random_array(h: usize, w: usize, seed: u64) -> Array2<f32> {
        // Simple xorshift for deterministic pseudo-random without extra deps.
        let mut s = seed.max(1);
        let mut x = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s as f32 / u64::MAX as f32) * 255.0
        };
        Array2::from_shape_fn((h, w), |_| x())
    }

    #[test]
    fn roundtrip_lossless_small() {
        let y = random_array(16, 16, 42);
        let (ll, lh, hl, hh) = forward(&y);
        let rec = inverse(&ll, &lh, &hl, &hh);
        let err = y
            .iter()
            .zip(rec.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-3, "max err {err}");
    }

    #[test]
    fn roundtrip_rectangular() {
        let y = random_array(32, 48, 7);
        let (ll, lh, hl, hh) = forward(&y);
        assert_eq!(ll.dim(), (16, 24));
        let rec = inverse(&ll, &lh, &hl, &hh);
        let err = y
            .iter()
            .zip(rec.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-3, "max err {err}");
    }

    #[test]
    fn constant_image_energy_in_ll() {
        let y = Array2::from_elem((8, 8), 100.0f32);
        let (ll, lh, hl, hh) = forward(&y);
        // Constant energy should concentrate in LL.
        assert!((ll[[0, 0]] - 200.0).abs() < 1e-3);
        assert!(lh.iter().all(|v| v.abs() < 1e-3));
        assert!(hl.iter().all(|v| v.abs() < 1e-3));
        assert!(hh.iter().all(|v| v.abs() < 1e-3));
    }
}
