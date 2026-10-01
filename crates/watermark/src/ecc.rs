//! Error correction + spatial-diversity tiling for capture robustness.
//!
//! WHY two layers instead of just bigger Δ:
//! - Random bit flips (JPEG, sensor noise) are independent per coefficient —
//!   a Hamming(7,4) code fixes any single flip per 7-bit word at only 75%
//!   overhead, no shared state, no Elias-Fano tables. It turns BER 5% into
//!   <0.5% in practice.
//! - Burst errors (cropping, occlusions, photo borders, EXIF rotation bands)
//!   wipe *contiguous* blocks. ECC alone cannot survive a lost quarter of
//!   the image. Tiling repeats the whole coded frame K times spread across
//!   the block grid; extraction majority-votes per position, so losing any
//!   minority of tiles still decodes. Copies are sequential across the grid
//!   (top-to-bottom), so a bottom crop kills late copies while early ones
//!   survive — a localized attack cannot hit the same bit position in every
//!   copy at once.
//!
//! Production upgrades: replace Hamming with LDPC/BCH + keyed interleaver
//! (spreads bursts pseudo-randomly instead of sequentially) and add soft
//! -decision decoding using reliability weights (see `qim::margin`).

/// Max copies of the frame spread across the block grid.
pub const MAX_TILES: usize = 8;

/// Hamming(7,4) encode: every 4 data bits -> 7 coded bits (p1,p2,d1,p3,d2,d3,d4).
/// Input length must be a multiple of 4.
pub fn hamming_encode(data: &[u8]) -> Vec<u8> {
    assert!(
        data.len().is_multiple_of(4),
        "hamming needs nibble-aligned input"
    );
    let mut out = Vec::with_capacity(data.len() / 4 * 7);
    for n in data.chunks(4) {
        let (d1, d2, d3, d4) = (n[0], n[1], n[2], n[3]);
        let p1 = d1 ^ d2 ^ d4;
        let p2 = d1 ^ d3 ^ d4;
        let p3 = d2 ^ d3 ^ d4;
        out.extend([p1, p2, d1, p3, d2, d3, d4]);
    }
    out
}

/// Hamming(7,4) decode with single-error correction.
/// Returns (data bits, words_corrected). Length must be multiple of 7;
/// trailing incomplete words are dropped.
pub fn hamming_decode(coded: &[u8]) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(coded.len() / 7 * 4);
    let mut corrected = 0;
    for w in coded.chunks(7) {
        if w.len() < 7 {
            break;
        }
        let (p1, p2, d1, p3, d2, d3, d4) = (w[0], w[1], w[2], w[3], w[4], w[5], w[6]);
        let s1 = p1 ^ d1 ^ d2 ^ d4;
        let s2 = p2 ^ d1 ^ d3 ^ d4;
        let s3 = p3 ^ d2 ^ d3 ^ d4;
        let syndrome = s1 as usize + (s2 as usize) * 2 + (s3 as usize) * 4;
        let mut fixed = [p1, p2, d1, p3, d2, d3, d4];
        if syndrome != 0 {
            // Syndrome indexes the flipped 1-based position.
            fixed[syndrome - 1] ^= 1;
            corrected += 1;
        }
        out.extend([fixed[2], fixed[4], fixed[5], fixed[6]]);
    }
    (out, corrected)
}

/// Repeat `frame` K times back-to-back (K>=1).
pub fn tile(frame: &[u8], k: usize) -> Vec<u8> {
    assert!(k >= 1);
    let mut out = Vec::with_capacity(frame.len() * k);
    for _ in 0..k {
        out.extend_from_slice(frame);
    }
    out
}

/// Per-position majority vote over K concatenated copies.
/// `raw.len()` must be >= frame_len; extra tail bits are ignored.
/// Ties resolve to 0. Returns (voted, tied_positions).
#[allow(clippy::needless_range_loop)]
pub fn majority_vote(raw: &[u8], frame_len: usize, k: usize) -> (Vec<u8>, usize) {
    assert!(frame_len > 0 && k >= 1);
    let mut out = vec![0u8; frame_len];
    let mut ties = 0;
    for i in 0..frame_len {
        let mut ones = 0;
        let mut present = 0;
        for copy in 0..k {
            if let Some(&b) = raw.get(copy * frame_len + i) {
                present += 1;
                ones += b as usize;
            }
        }
        if present == 0 {
            continue;
        }
        if ones * 2 == present {
            ties += 1; // exact tie -> 0, flagged as unreliable
        }
        out[i] = u8::from(ones * 2 > present);
    }
    (out, ties)
}

/// How many tiles fit: at least 1, at most MAX_TILES.
/// `tiles_arg`: 0 = auto-fill, else exact request (errors if it fits <1).
pub fn tile_count(capacity: usize, frame_len: usize, tiles_arg: usize) -> Result<usize, String> {
    if frame_len == 0 {
        return Err("nothing to embed".into());
    }
    if tiles_arg == 0 {
        Ok((capacity / frame_len).clamp(1, MAX_TILES))
    } else if capacity / frame_len >= tiles_arg {
        Ok(tiles_arg)
    } else {
        Err(format!(
            "payload too large for --tiles {tiles_arg}: need {} bits, capacity {capacity}",
            frame_len * tiles_arg
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamming_roundtrip() {
        let data: Vec<u8> = (0..32).map(|i| (i % 2) as u8).collect();
        let coded = hamming_encode(&data);
        assert_eq!(coded.len(), 56);
        let (dec, corr) = hamming_decode(&coded);
        assert_eq!(dec, data);
        assert_eq!(corr, 0);
    }

    #[test]
    fn hamming_corrects_single_flip_per_word() {
        let data = vec![1, 0, 1, 1, 0, 0, 1, 0];
        let mut coded = hamming_encode(&data);
        // Flip one bit in each 7-bit word.
        coded[2] ^= 1;
        coded[9] ^= 1;
        let (dec, corr) = hamming_decode(&coded);
        assert_eq!(dec, data);
        assert_eq!(corr, 2);
    }

    #[test]
    fn vote_recovers_with_minority_corruption() {
        let frame = vec![1, 0, 1, 1, 0, 0, 0, 1];
        let mut tiled = tile(&frame, 3);
        // Corrupt one whole copy: every position still has 2-of-3 intact.
        for i in 0..8 {
            tiled[16 + i] ^= 1;
        }
        let (voted, _) = majority_vote(&tiled, frame.len(), 3);
        assert_eq!(voted, frame);
    }

    #[test]
    fn vote_tie_goes_zero() {
        let (voted, ties) = majority_vote(&[1, 0], 1, 2);
        assert_eq!((voted, ties), (vec![0], 1));
    }

    #[test]
    fn tile_count_auto_and_manual() {
        assert_eq!(tile_count(1024, 178, 0).unwrap(), 5);
        assert_eq!(tile_count(100, 178, 0).unwrap(), 1);
        assert_eq!(tile_count(1024, 178, 3).unwrap(), 3);
        assert!(tile_count(100, 178, 3).is_err());
    }
}
