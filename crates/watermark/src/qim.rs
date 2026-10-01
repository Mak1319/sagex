//! Quantization Index Modulation (QIM) embed/extract.
//!
//! WHY QIM over additive spread-spectrum: additive methods (`y = x + α·w`)
//! suffer *host interference* — the cover image itself acts as noise at
//! the detector, forcing higher α (more visible) for the same robustness.
//! QIM instead *quantizes* the host coefficient to a bit-dependent lattice
//! (even multiples of Δ for 0, odd for 1). Detection is a nearest-lattice
//! lookup, so the host no longer interferes — robustness scales directly
//! with Δ. Larger Δ survives stronger JPEG/noise but moves coefficients
//! further (more visible): that is the explicit robustness-vs-
//! imperceptibility knob exposed as `--delta`.
//!
//! We use uniform scalar QIM (Costa's "writing on dirty paper" in its
//! simplest form). Production upgrades: spread-transform QIM (project a
//! vector of coeffs onto a random keyed direction first) + ECC.

/// Embed one bit into a coefficient with step `delta`.
pub fn embed(coeff: f32, bit: u8, delta: f32) -> f32 {
    debug_assert!(delta > 0.0);
    let q = (coeff / delta).round() as i32;
    let want_parity = (bit & 1) as i32;
    // Rust `%` keeps sign; use euclidean parity so -3 -> 1, -2 -> 0.
    let parity = q.rem_euclid(2);
    if parity == want_parity {
        q as f32 * delta
    } else {
        // Move ±1 to the lattice point with correct parity that is closest.
        let up = (q + 1) as f32 * delta;
        let down = (q - 1) as f32 * delta;
        if (coeff - up).abs() <= (coeff - down).abs() {
            up
        } else {
            down
        }
    }
}

/// Extract one bit: nearest lattice parity.
pub fn extract(coeff: f32, delta: f32) -> u8 {
    debug_assert!(delta > 0.0);
    ((coeff / delta).round() as i32).rem_euclid(2) as u8
}

/// Signed distance to the decision boundary (confidence proxy).
/// Large positive => far from boundary (confident); near 0 => fragile.
#[allow(dead_code)]
pub fn margin(coeff: f32, delta: f32) -> f32 {
    let t = coeff / delta;
    let frac = (t - t.round()).abs();
    // Boundary sits at half-integers; margin = |0.5 - frac| * Δ.
    (0.5 - frac).abs() * delta
}

/// Embed a whole bit-slice into a coefficient slice (truncates to min len).
#[allow(dead_code)]
pub fn embed_slice(coeffs: &mut [f32], bits: &[u8], delta: f32) {
    for (c, b) in coeffs.iter_mut().zip(bits.iter()) {
        *c = embed(*c, *b, delta);
    }
}

/// Extract bits from a coefficient slice.
#[allow(dead_code)]
pub fn extract_slice(coeffs: &[f32], delta: f32) -> Vec<u8> {
    coeffs.iter().map(|&c| extract(c, delta)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_clean_across_deltas() {
        let coeffs = [-50.0, -17.3, -0.5, 0.0, 0.7, 12.4, 33.9, 100.0];
        for &delta in &[4.0f32, 8.0, 12.0, 20.0] {
            for &bit in &[0u8, 1] {
                for &c in &coeffs {
                    let w = embed(c, bit, delta);
                    assert_eq!(extract(w, delta), bit, "c={c} d={delta} b={bit}");
                    // Worst-case move is one full lattice step Δ (e.g. host sits
                    // exactly on the wrong-parity lattice point).
                    assert!((w - c).abs() <= delta + 1e-4);
                }
            }
        }
    }

    #[test]
    fn roundtrip_array_with_varied_delta() {
        let coeffs: Vec<f32> = (0..64).map(|i| i as f32 * 1.7 - 50.0).collect();
        let bits: Vec<u8> = (0..64).map(|i| (i % 2) as u8).collect();
        for &delta in &[6.0f32, 12.0, 25.0] {
            let mut w = coeffs.clone();
            embed_slice(&mut w, &bits, delta);
            let got = extract_slice(&w, delta);
            assert_eq!(got, bits, "delta {delta}");
        }
    }

    #[test]
    fn noise_robustness_scales_with_delta() {
        // Fixed noise σ=2: small Δ should err more than large Δ.
        // Deterministic pseudo-noise (no rand dep).
        let coeffs = vec![20.0f32; 64];
        let bits: Vec<u8> = (0..64).map(|i| (i % 2) as u8).collect();
        let noise: Vec<f32> = (0..64)
            .map(|i| ((i * 37 % 11) as f32 - 5.0) * 0.4)
            .collect();
        let mut errs = vec![];
        for &delta in &[6.0f32, 24.0] {
            let mut w = coeffs.clone();
            embed_slice(&mut w, &bits, delta);
            let noisy: Vec<f32> = w.iter().zip(noise.iter()).map(|(a, b)| a + b).collect();
            let got = extract_slice(&noisy, delta);
            let e = got.iter().zip(bits.iter()).filter(|(a, b)| a != b).count();
            errs.push(e);
        }
        assert!(errs[0] >= errs[1], "errs {errs:?}");
        assert_eq!(errs[1], 0);
    }
}
