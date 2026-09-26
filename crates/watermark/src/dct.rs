//! Blockwise 8x8 DCT-II / DCT-III (orthonormal) implemented manually.
//!
//! WHY DCT on the LL subband: DWT-LL already gives robustness against
//! filtering; DCT then compacts each block's remaining energy into a few
//! low-frequency coefficients, leaving a stable "mid-frequency" region
//! to hide in. WHY mid-frequency specifically:
//! - DC (0,0) and very-low AC: huge perceptual impact (block brightness),
//!   and targeted by luminance quantization — changing them is visible.
//! - High-frequency AC: wiped out first by JPEG quantization and
//!   low-pass/denoise attacks — watermark dies with them.
//! - Mid-frequency: the sweet spot — invisible enough, survives JPEG
//!   at quality >= ~70 when QIM step Δ exceeds the JPEG quant step.
//!
//! We precompute the orthonormal DCT matrix C so forward = C·B·Cᵀ and
//! inverse = Cᵀ·B·C, lossless up to float error.

use ndarray::Array2;

/// Mid-band coefficient positions carrying payload bits (4 bits/block).
/// Chosen away from DC(0,0) and from the high-frequency corner (7,7).
/// Capacity = num_8x8_blocks(LL) * MIDBAND.len().
pub const MIDBAND: [(usize, usize); 4] = [(1, 2), (2, 1), (2, 3), (3, 2)];

/// Number of payload bits one LL plane can hold.
pub fn capacity_bits(ll_h: usize, ll_w: usize) -> usize {
    (ll_h / 8) * (ll_w / 8) * MIDBAND.len()
}

#[allow(clippy::needless_range_loop)]
fn dct_matrix() -> [[f32; 8]; 8] {
    let mut c = [[0.0f32; 8]; 8];
    for u in 0..8 {
        let alpha = if u == 0 {
            (1.0f32 / 8.0).sqrt()
        } else {
            (2.0f32 / 8.0).sqrt()
        };
        for x in 0..8 {
            c[u][x] = alpha * (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos());
        }
    }
    c
}

/// Forward DCT-II on one 8x8 block: out = C * block * Cᵀ.
#[allow(clippy::needless_range_loop)]
pub fn forward_8x8(block: &[[f32; 8]; 8]) -> [[f32; 8]; 8] {
    let c = dct_matrix();
    // tmp = C * block: tmp[u][x] = sum_k C[u][k]*block[k][x]
    let mut tmp = [[0.0f32; 8]; 8];
    for u in 0..8 {
        for x in 0..8 {
            let mut s = 0.0;
            for k in 0..8 {
                s += c[u][k] * block[k][x];
            }
            tmp[u][x] = s;
        }
    }
    // out = tmp * Cᵀ: out[u][v] = sum_k tmp[u][k]*C[v][k]
    let mut out = [[0.0f32; 8]; 8];
    for u in 0..8 {
        for v in 0..8 {
            let mut s = 0.0;
            for k in 0..8 {
                s += tmp[u][k] * c[v][k];
            }
            out[u][v] = s;
        }
    }
    out
}

/// Inverse DCT-III on one 8x8 block: block = Cᵀ * coeffs * C.
#[allow(clippy::needless_range_loop)]
pub fn inverse_8x8(coeffs: &[[f32; 8]; 8]) -> [[f32; 8]; 8] {
    let c = dct_matrix();
    // tmp = Cᵀ * coeffs: tmp[k][v] = sum_u C[u][k]*coeffs[u][v]
    let mut tmp = [[0.0f32; 8]; 8];
    for k in 0..8 {
        for v in 0..8 {
            let mut s = 0.0;
            for u in 0..8 {
                s += c[u][k] * coeffs[u][v];
            }
            tmp[k][v] = s;
        }
    }
    // out = tmp * C: out[y][x] = sum_v tmp[y][v]*C[v][x]
    let mut out = [[0.0f32; 8]; 8];
    for y in 0..8 {
        for x in 0..8 {
            let mut s = 0.0;
            for v in 0..8 {
                s += tmp[y][v] * c[v][x];
            }
            out[y][x] = s;
        }
    }
    out
}

fn get_block(m: &Array2<f32>, by: usize, bx: usize) -> [[f32; 8]; 8] {
    let mut b = [[0.0f32; 8]; 8];
    for y in 0..8 {
        for x in 0..8 {
            b[y][x] = m[[by * 8 + y, bx * 8 + x]];
        }
    }
    b
}

fn put_block(m: &mut Array2<f32>, by: usize, bx: usize, b: &[[f32; 8]; 8]) {
    for y in 0..8 {
        for x in 0..8 {
            m[[by * 8 + y, bx * 8 + x]] = b[y][x];
        }
    }
}

/// Forward blockwise DCT. Dims must be multiples of 8.
pub fn forward_image(ll: &Array2<f32>) -> Array2<f32> {
    let (h, w) = ll.dim();
    assert!(h % 8 == 0 && w % 8 == 0);
    let mut out = ll.clone();
    for by in 0..h / 8 {
        for bx in 0..w / 8 {
            let b = get_block(ll, by, bx);
            put_block(&mut out, by, bx, &forward_8x8(&b));
        }
    }
    out
}

/// Inverse blockwise DCT.
pub fn inverse_image(coeffs: &Array2<f32>) -> Array2<f32> {
    let (h, w) = coeffs.dim();
    assert!(h % 8 == 0 && w % 8 == 0);
    let mut out = coeffs.clone();
    for by in 0..h / 8 {
        for bx in 0..w / 8 {
            let b = get_block(coeffs, by, bx);
            put_block(&mut out, by, bx, &inverse_8x8(&b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_single_block() {
        let mut b = [[0.0f32; 8]; 8];
        for y in 0..8 {
            for x in 0..8 {
                b[y][x] = (y * 8 + x) as f32;
            }
        }
        let c = forward_8x8(&b);
        let rec = inverse_8x8(&c);
        for y in 0..8 {
            for x in 0..8 {
                assert!((b[y][x] - rec[y][x]).abs() < 1e-3, "at {y},{x}");
            }
        }
    }

    #[test]
    fn roundtrip_image_16x16() {
        let m = Array2::from_shape_fn((16, 16), |(y, x)| (y * 16 + x) as f32 % 255.0);
        let c = forward_image(&m);
        let rec = inverse_image(&c);
        let err = m
            .iter()
            .zip(rec.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-2, "max err {err}");
    }

    #[test]
    fn dc_of_constant_block() {
        let b = [[100.0f32; 8]; 8];
        let c = forward_8x8(&b);
        // Orthonormal DC = 8 * mean? For constant v: DC = 8*v.
        assert!((c[0][0] - 800.0).abs() < 1e-2);
        assert!(c[0][1].abs() < 1e-2 && c[1][0].abs() < 1e-2);
    }
}
