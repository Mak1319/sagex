//! CLI entry: `watermark embed` / `watermark extract`.
//!
//! Pipeline recap (see module docs for WHY):
//! embed: RGB -> YCbCr -> pad16 -> DFT-sync-ring -> DWT(Haar) -> DCT(8x8 on LL)
//!   -> QIM(mid-band, Δ) -> IDCT -> IDWT -> DFT already in -> merge CbCr -> RGB.
//! extract: RGB -> DFT detect/log-polar -> inverse warp -> pad -> DWT -> DCT -> QIM detect.

mod attacks;
mod dct;
mod dft_sync;
mod dwt;
mod ecc;
mod qim;

use clap::{Parser, Subcommand};
use image::{Rgb, RgbImage};
use ndarray::Array2;

// ---------------------------------------------------------------- CLI ---

#[derive(Parser)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[command(name = "watermark", about = "Hybrid DWT-DCT-DFT image watermarking")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Cmd {
    /// Embed a payload into a cover image.
    Embed {
        #[arg(long)]
        input: String,
        #[arg(long)]
        output: String,
        /// Payload string (unless it names an existing file — then file bytes are used).
        #[arg(long, default_value = "")]
        payload: String,
        /// Explicit payload file (overrides --payload).
        #[arg(long)]
        payload_file: Option<String>,
        /// QIM step Δ. Larger = more robust, more visible.
        #[arg(long, alias = "strength", default_value_t = 12.0)]
        delta: f32,
        /// Tile copies across the block grid: 0 = auto-fill (up to 8).
        /// More tiles = better crop survival, less capacity for long messages.
        #[arg(long, default_value_t = 0)]
        tiles: usize,
        /// Disable Hamming(7,4) error correction (needed only to read
        /// pre-hardening images; combine with --tiles 1 for exact legacy).
        #[arg(long, default_value_t = false)]
        no_ecc: bool,
        /// Self-test: e.g. "rotate:5,scale:0.9,crop:0.05,bright:1.1,jpeg:80".
        #[arg(long)]
        test_attack: Option<String>,
    },
    /// Extract a payload from a (possibly attacked) image.
    Extract {
        #[arg(long)]
        input: String,
        /// Number of *payload* bits (message bits, excluding framing/ECC).
        #[arg(long)]
        payload_len: usize,
        #[arg(long, alias = "strength", default_value_t = 12.0)]
        delta: f32,
        /// Must match the embed side (0 = auto from image capacity).
        /// For photos/crops with different dims, pass the embed's tile count.
        #[arg(long, default_value_t = 0)]
        tiles: usize,
        /// Must match the embed side.
        #[arg(long, default_value_t = false)]
        no_ecc: bool,
        /// Expected message for BER (string or existing file path).
        #[arg(long)]
        expected: Option<String>,
        #[arg(long)]
        expected_file: Option<String>,
        /// Optional path to write recovered bytes.
        #[arg(long)]
        output: Option<String>,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Embed {
            input,
            output,
            payload,
            payload_file,
            delta,
            tiles,
            no_ecc,
            test_attack,
        } => {
            check_delta(delta)?;
            let msg = load_payload(&payload, payload_file.as_deref())?;
            if msg.is_empty() {
                return Err("payload is empty".into());
            }
            if !no_ecc && msg.len() > u16::MAX as usize {
                return Err("payload too long for 16-bit length header".into());
            }
            let cover = image::open(&input)
                .map_err(|e| format!("open {input}: {e}"))?
                .to_rgb8();
            let (wm, psnr_db, cap, tiles_used, frame_len) =
                embed_image(&cover, &msg, delta, tiles, !no_ecc)?;
            // Crop happens inside; save by extension.
            wm.save(&output)
                .map_err(|e| format!("save {output}: {e}"))?;
            println!(
                "embedded {} payload bits ({} framed x{tiles_used}) | capacity {cap} bits | PSNR {psnr_db:.2} dB | Δ={delta} ecc={}",
                msg.len() * 8,
                frame_len,
                !no_ecc
            );
            println!("saved {output}");

            if let Some(spec_str) = test_attack {
                let spec = attacks::parse(&spec_str)?;
                let attacked = attacks::apply_all(&wm, &spec);
                let atk_path = sibling_path(&output, "attacked");
                attacked.save(&atk_path).map_err(|e| e.to_string())?;
                // Extract from attacked in-memory with the same format.
                let (raw, det) = extract_raw(&attacked, frame_len * tiles_used, delta, false)?;
                let (voted, tied) = ecc::majority_vote(&raw, frame_len, tiles_used);
                let reference = if no_ecc {
                    frame_payload(&msg)
                } else {
                    frame_coded(&msg)
                };
                let (errs, ber) = attacks::ber_bits(&reference, &voted);
                println!(
                    "attack [{spec_str}] -> {atk_path} | voted BER {ber:.4} ({errs}/{}) ties={tied} sync_conf={:.3}",
                    voted.len(),
                    det.confidence
                );
                // Retry with lighting flattening (photo-gradient fallback).
                let (raw2, _) = extract_raw(&attacked, frame_len * tiles_used, delta, true)?;
                let (voted2, _) = ecc::majority_vote(&raw2, frame_len, tiles_used);
                let (errs2, ber2) = attacks::ber_bits(&reference, &voted2);
                if ber2 < ber {
                    println!(
                        "  with lighting-flatten: BER {ber2:.4} ({errs2}/{}) (better)",
                        voted2.len()
                    );
                }
                let show = if ber2 < ber { voted2 } else { voted };
                if no_ecc {
                    match unframe(&show) {
                        Ok((bytes, ok)) => println!(
                            "recovered after attack: {:?} checksum_ok={ok}",
                            String::from_utf8_lossy(&bytes)
                        ),
                        Err(e) => println!("unframe after attack failed: {e}"),
                    }
                } else {
                    match unframe_coded(&show) {
                        Ok((bytes, ok, corr)) => println!(
                            "recovered after attack: {:?} checksum_ok={ok} ecc_corrected_words={corr}",
                            String::from_utf8_lossy(&bytes)
                        ),
                        Err(e) => println!("unframe after attack failed: {e}"),
                    }
                }
            }
            Ok(())
        }
        Cmd::Extract {
            input,
            payload_len,
            delta,
            tiles,
            no_ecc,
            expected,
            expected_file,
            output,
        } => {
            check_delta(delta)?;
            if payload_len % 8 != 0 {
                return Err(format!(
                    "payload_len {payload_len} must be a multiple of 8 in ecc mode (use --no-ecc for legacy bit-exact reads)"
                ));
            }
            let img = image::open(&input)
                .map_err(|e| format!("open {input}: {e}"))?
                .to_rgb8();
            // Frame length in the coded domain.
            let frame_len = if no_ecc {
                24 + payload_len
            } else {
                16 + payload_len / 4 * 7 + 8
            };
            let cap = capacity_of(&img);
            let tiles_used = ecc::tile_count(cap, frame_len, tiles)?;
            let total = frame_len * tiles_used;
            // Try raw, then lighting-flattened retry; prefer checksum_ok.
            let (raw, det) = extract_raw(&img, total, delta, false)?;
            let (voted, tied) = ecc::majority_vote(&raw, frame_len, tiles_used);
            let (raw_f, _) = extract_raw(&img, total, delta, true)?;
            let (voted_f, tied_f) = ecc::majority_vote(&raw_f, frame_len, tiles_used);
            println!(
                "sync estimate: θ={:.2}° scale={:.4} conf={:.3} | tiles={tiles_used} ecc={} ties={tied}/{tied_f}",
                det.theta_deg,
                det.scale,
                det.confidence,
                !no_ecc
            );
            if no_ecc {
                let r1 = unframe(&voted);
                let r2 = unframe(&voted_f);
                // Prefer the checksum-passing decode; else the raw one.
                let (bits_best, which) = match (&r1, &r2) {
                    (Ok((_, true)), _) => (voted, "raw"),
                    (_, Ok((_, true))) => (voted_f, "flattened"),
                    _ => (voted, "raw"),
                };
                match unframe(&bits_best) {
                    Ok((bytes, ok)) => {
                        println!(
                            "payload ({} bytes, checksum_ok={ok}, via {which}):",
                            bytes.len()
                        );
                        println!("{}", String::from_utf8_lossy(&bytes));
                        if let Some(p) = output {
                            std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
                            println!("wrote recovered bytes to {p}");
                        }
                        report_ber(
                            expected,
                            expected_file,
                            &bytes,
                            payload_len,
                            &bits_best,
                            true,
                        )?;
                        if !ok {
                            eprintln!("warning: checksum mismatch — payload likely corrupted");
                        }
                    }
                    Err(e) => return Err(format!("unframe: {e}")),
                }
            } else {
                let r1 = unframe_coded(&voted);
                let r2 = unframe_coded(&voted_f);
                let (bits_best, which) = match (&r1, &r2) {
                    (Ok((_, true, _)), _) => (voted, "raw"),
                    (_, Ok((_, true, _))) => (voted_f, "flattened"),
                    _ => (voted, "raw"),
                };
                match unframe_coded(&bits_best) {
                    Ok((bytes, ok, corr)) => {
                        println!(
                            "payload ({} bytes, checksum_ok={ok}, ecc_corrected_words={corr}, via {which}):",
                            bytes.len()
                        );
                        println!("{}", String::from_utf8_lossy(&bytes));
                        if let Some(p) = output {
                            std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
                            println!("wrote recovered bytes to {p}");
                        }
                        report_ber(
                            expected,
                            expected_file,
                            &bytes,
                            payload_len,
                            &bits_best,
                            false,
                        )?;
                        if !ok {
                            eprintln!("warning: checksum mismatch — payload likely corrupted");
                        }
                    }
                    Err(e) => return Err(format!("unframe: {e}")),
                }
            }
            Ok(())
        }
    }
}

/// BER of the recovered *message* bits vs an expected reference.
fn report_ber(
    expected: Option<String>,
    expected_file: Option<String>,
    bytes: &[u8],
    payload_len: usize,
    voted_framed: &[u8],
    legacy: bool,
) -> Result<(), String> {
    let exp = load_expected(expected, expected_file.as_deref())?;
    if let Some(exp_bytes) = exp {
        let exp_bits = bytes_to_bits(&exp_bytes);
        let got_bits = bytes_to_bits(bytes);
        let n = exp_bits.len().min(got_bits.len()).min(payload_len);
        let (e, ber) = attacks::ber_bits(&exp_bits[..n], &got_bits[..n]);
        println!("BER vs expected: {ber:.4} ({e}/{n})");
        println!(
            "checksum state above is the confidence signal (framed_bits={}{})",
            voted_framed.len(),
            if legacy { ", legacy" } else { "" }
        );
    }
    Ok(())
}

fn check_delta(d: f32) -> Result<(), String> {
    if !(d > 0.0 && d.is_finite()) {
        return Err(format!("--delta must be > 0 (got {d})"));
    }
    Ok(())
}

// ---------------------------------------------------------- payload ---

fn load_payload(s: &str, file: Option<&str>) -> Result<Vec<u8>, String> {
    if let Some(p) = file {
        return std::fs::read(p).map_err(|e| format!("read {p}: {e}"));
    }
    if !s.is_empty() && std::path::Path::new(s).is_file() {
        // Auto-detect: --payload naming an existing file means file bytes.
        return std::fs::read(s).map_err(|e| format!("read {s}: {e}"));
    }
    Ok(s.as_bytes().to_vec())
}

fn load_expected(s: Option<String>, file: Option<&str>) -> Result<Option<Vec<u8>>, String> {
    if let Some(p) = file {
        return Ok(Some(
            std::fs::read(p).map_err(|e| format!("read {p}: {e}"))?,
        ));
    }
    if let Some(v) = s {
        if !v.is_empty() && std::path::Path::new(&v).is_file() {
            return Ok(Some(
                std::fs::read(&v).map_err(|e| format!("read {v}: {e}"))?,
            ));
        }
        if !v.is_empty() {
            return Ok(Some(v.into_bytes()));
        }
    }
    Ok(None)
}

fn bytes_to_bits(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() * 8);
    for &b in bytes {
        for i in (0..8).rev() {
            out.push((b >> i) & 1);
        }
    }
    out
}

fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.len() / 8);
    for ch in bits.chunks(8) {
        if ch.len() < 8 {
            break;
        }
        let mut b = 0u8;
        for &b2 in ch {
            b = (b << 1) | (b2 & 1);
        }
        out.push(b);
    }
    out
}

/// Frame: [16-bit BE payload-byte-len][payload bits][8-bit wrapping-sum checksum].
fn frame_payload(msg: &[u8]) -> Vec<u8> {
    let mut bits = Vec::with_capacity(24 + msg.len() * 8);
    let len = msg.len() as u16;
    for i in (0..16).rev() {
        bits.push(((len >> i) & 1) as u8);
    }
    bits.extend(bytes_to_bits(msg));
    let chk = msg.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    for i in (0..8).rev() {
        bits.push((chk >> i) & 1);
    }
    bits
}

fn unframe(bits: &[u8]) -> Result<(Vec<u8>, bool), String> {
    if bits.len() < 24 {
        return Err(format!("need ≥24 bits, got {}", bits.len()));
    }
    let mut len: usize = 0;
    for &b in &bits[0..16] {
        len = (len << 1) | (b as usize);
    }
    let need = 16 + len * 8 + 8;
    if bits.len() < need {
        return Err(format!(
            "framing says {len} bytes but only {} bits present",
            bits.len()
        ));
    }
    let payload_bits = &bits[16..16 + len * 8];
    let bytes = bits_to_bytes(payload_bits);
    let mut chk: usize = 0;
    for &b in &bits[16 + len * 8..need] {
        chk = (chk << 1) | (b as usize);
    }
    let ok = bytes.iter().fold(0u8, |a, &b| a.wrapping_add(b)) as usize == chk;
    Ok((bytes, ok))
}

// ------------------------------------------------------------ color ---

fn rgb_to_ycbcr(img: &RgbImage) -> (Array2<f32>, Array2<f32>, Array2<f32>) {
    let (w, h) = img.dimensions();
    let mut y = Array2::<f32>::zeros((h as usize, w as usize));
    let mut cb = Array2::<f32>::zeros((h as usize, w as usize));
    let mut cr = Array2::<f32>::zeros((h as usize, w as usize));
    for yy in 0..h {
        for xx in 0..w {
            let p = img.get_pixel(xx, yy).0;
            let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
            y[[yy as usize, xx as usize]] = 0.299 * r + 0.587 * g + 0.114 * b;
            cb[[yy as usize, xx as usize]] = 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b;
            cr[[yy as usize, xx as usize]] = 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b;
        }
    }
    (y, cb, cr)
}

fn ycbcr_to_rgb(y: &Array2<f32>, cb: &Array2<f32>, cr: &Array2<f32>) -> RgbImage {
    let (h, w) = (y.nrows(), y.ncols());
    let mut img = RgbImage::new(w as u32, h as u32);
    for yy in 0..h {
        for xx in 0..w {
            let yyv = y[[yy, xx]];
            let cbv = cb[[yy, xx]] - 128.0;
            let crv = cr[[yy, xx]] - 128.0;
            let r = (yyv + 1.402 * crv).clamp(0.0, 255.0) as u8;
            let g = (yyv - 0.344136 * cbv - 0.714136 * crv).clamp(0.0, 255.0) as u8;
            let b = (yyv + 1.772 * cbv).clamp(0.0, 255.0) as u8;
            img.put_pixel(xx as u32, yy as u32, Rgb([r, g, b]));
        }
    }
    img
}

fn pad16(m: &Array2<f32>) -> (Array2<f32>, usize, usize) {
    let (h, w) = m.dim();
    let ph = h.div_ceil(16) * 16;
    let pw = w.div_ceil(16) * 16;
    if ph == h && pw == w {
        return (m.clone(), h, w);
    }
    let mut out = Array2::<f32>::zeros((ph, pw));
    for r in 0..ph {
        for c in 0..pw {
            out[[r, c]] = m[[r.min(h - 1), c.min(w - 1)]]; // edge replicate
        }
    }
    (out, h, w)
}

fn crop(m: &Array2<f32>, h: usize, w: usize) -> Array2<f32> {
    m.slice(ndarray::s![0..h, 0..w]).to_owned()
}

// ---------------------------------------------------------- pipeline ---

/// Usable QIM bit capacity of an RGB image of these dimensions.
fn capacity_of(img: &RgbImage) -> usize {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let ph = h.div_ceil(16) * 16;
    let pw = w.div_ceil(16) * 16;
    dct::capacity_bits(ph / 2, pw / 2)
}

/// Coded frame: [16-bit BE msg-byte-len][Hamming(7,4) payload][8-bit checksum].
/// WHY code only the payload: the length header is read before we know L,
/// so it stays repetition-protected (via tiling) but uncoded — a flipped
/// header bit is caught by the capacity sanity check + checksum instead.
fn frame_coded(msg: &[u8]) -> Vec<u8> {
    let mut bits = Vec::with_capacity(16 + msg.len() * 14 + 8);
    let len = msg.len() as u16;
    for i in (0..16).rev() {
        bits.push(((len >> i) & 1) as u8);
    }
    bits.extend(ecc::hamming_encode(&bytes_to_bits(msg)));
    let chk = msg.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    for i in (0..8).rev() {
        bits.push((chk >> i) & 1);
    }
    bits
}

/// Decode a voted coded frame. Returns (bytes, checksum_ok, words_corrected).
fn unframe_coded(bits: &[u8]) -> Result<(Vec<u8>, bool, usize), String> {
    if bits.len() < 24 {
        return Err(format!("need ≥24 bits, got {}", bits.len()));
    }
    let mut len: usize = 0;
    for &b in &bits[0..16] {
        len = (len << 1) | (b as usize);
    }
    if len > 4096 {
        return Err(format!(
            "framing says {len} bytes — likely desync, not data"
        ));
    }
    let need = 16 + len * 14 + 8; // 8 payload bits -> 14 coded bits
    if bits.len() < need {
        return Err(format!(
            "framing says {len} bytes but only {} bits present",
            bits.len()
        ));
    }
    let (payload_bits, corrected) = ecc::hamming_decode(&bits[16..16 + len * 14]);
    let bytes = bits_to_bytes(&payload_bits);
    let mut chk: usize = 0;
    for &b in &bits[16 + len * 14..need] {
        chk = (chk << 1) | (b as usize);
    }
    let ok = bytes.iter().fold(0u8, |a, &b| a.wrapping_add(b)) as usize == chk;
    Ok((bytes, ok, corrected))
}

/// Remove slow lighting gradients (photo falloff, screen vignetting) while
/// keeping block-rate detail: y_flat = y − boxblur(y,16) + mean(y).
/// WHY this is safe for QIM: the blur kernel is ~33px wide, so its frequency
/// response is near-zero at the mid-band DCT frequencies (~2–4 cycles per
/// 8px block). It strips DC + very-low AC (where gradients live) and leaves
/// the hidden coefficients almost untouched on clean images, while rescuing
/// reads under gradients that would otherwise bias every quantizer.
fn flatten_lighting(y: &Array2<f32>) -> Array2<f32> {
    const R: isize = 16;
    let (h, w) = y.dim();
    let mean = y.iter().sum::<f32>() / y.len() as f32;
    // Separable box blur via row then column passes.
    let mut tmp = Array2::<f32>::zeros((h, w));
    for r in 0..h {
        for c in 0..w {
            let mut s = 0.0;
            let mut n = 0;
            let c0 = c as isize - R;
            let c1 = c as isize + R;
            for cc in c0..=c1 {
                if cc >= 0 && cc < w as isize {
                    s += y[[r, cc as usize]];
                    n += 1;
                }
            }
            tmp[[r, c]] = s / n.max(1) as f32;
        }
    }
    let mut out = Array2::<f32>::zeros((h, w));
    for r in 0..h {
        for c in 0..w {
            let mut s = 0.0;
            let mut n = 0;
            for rr in r as isize - R..=r as isize + R {
                if rr >= 0 && rr < h as isize {
                    s += tmp[[rr as usize, c]];
                    n += 1;
                }
            }
            out[[r, c]] = (y[[r, c]] - s / n.max(1) as f32 + mean).clamp(0.0, 255.0);
        }
    }
    out
}

/// Full embed on an RGB image.
/// Returns (watermarked_rgb, psnr_db, capacity_bits, tiles_used, frame_len).
fn embed_image(
    cover: &RgbImage,
    msg: &[u8],
    delta: f32,
    tiles_arg: usize,
    use_ecc: bool,
) -> Result<(RgbImage, f32, usize, usize, usize), String> {
    let (w0, h0) = (cover.width() as usize, cover.height() as usize);
    if w0 < 32 || h0 < 32 {
        return Err("image too small (need ≥32x32)".into());
    }
    let (y0, cb0, cr0) = rgb_to_ycbcr(cover);
    let (mut y, oh, ow) = pad16(&y0);
    let (cb, _, _) = pad16(&cb0);
    let (cr, _, _) = pad16(&cr0);

    // 1) Sync template first so final QIM quantization is exact.
    dft_sync::embed_sync(&mut y, dft_sync::DEFAULT_STRENGTH);

    // 2) DWT -> DCT(LL).
    let (mut ll, lh, hl, hh) = dwt::forward(&y);
    let (ll_h, ll_w) = ll.dim();
    if ll_h % 8 != 0 || ll_w % 8 != 0 {
        return Err("internal: LL dims not multiple of 8".into());
    }
    let cap = dct::capacity_bits(ll_h, ll_w);
    let frame: Vec<u8> = if use_ecc {
        frame_coded(msg)
    } else {
        frame_payload(msg)
    };
    let tiles_used = ecc::tile_count(cap, frame.len(), tiles_arg)?;
    let tiled = ecc::tile(&frame, tiles_used);
    let mut coeffs = dct::forward_image(&ll);

    // 3) QIM into mid-band positions, block order (by,bx,MIDBAND).
    let mut k = 0;
    'outer: for by in 0..ll_h / 8 {
        for bx in 0..ll_w / 8 {
            for &(u, v) in &dct::MIDBAND {
                if k >= tiled.len() {
                    break 'outer;
                }
                let c = coeffs[[by * 8 + u, bx * 8 + v]];
                coeffs[[by * 8 + u, bx * 8 + v]] = qim::embed(c, tiled[k], delta);
                k += 1;
            }
        }
    }

    // 4) Invert.
    ll = dct::inverse_image(&coeffs);
    let y_wm = dwt::inverse(&ll, &lh, &hl, &hh);

    let psnr = psnr_y(&y, &y_wm);
    let y_final = crop(&y_wm, oh.min(y_wm.nrows()), ow.min(y_wm.ncols()));
    let cb_final = crop(&cb, oh, ow);
    let cr_final = crop(&cr, oh, ow);
    let frame_len = frame.len();
    Ok((
        ycbcr_to_rgb(&y_final, &cb_final, &cr_final),
        psnr,
        cap,
        tiles_used,
        frame_len,
    ))
}

/// Extract `total_bits` of raw (still-tiled) QIM bits; resyncs geometry
/// first. Set `flatten` to pre-strip lighting gradients (retry path).
fn extract_raw(
    img: &RgbImage,
    total_bits: usize,
    delta: f32,
    flatten: bool,
) -> Result<(Vec<u8>, dft_sync::Detection), String> {
    // Geometry resync on RGB (keeps dims).
    let (y_raw, _, _) = rgb_to_ycbcr(img);
    let det = dft_sync::detect(&y_raw);
    let mut synced = img.clone();
    if det.confidence >= 0.08 && (det.theta_deg.abs() > 0.2 || (det.scale - 1.0).abs() > 0.005) {
        if (det.scale - 1.0).abs() > 0.005 {
            synced = attacks::apply_scale(&synced, 1.0 / det.scale);
        }
        if det.theta_deg.abs() > 0.2 {
            synced = attacks::apply_rotate(&synced, -det.theta_deg);
        }
    }
    let (mut y_s, _, _) = rgb_to_ycbcr(&synced);
    if flatten {
        y_s = flatten_lighting(&y_s);
    }
    let (y_p, _, _) = pad16(&y_s);
    let (ll, _, _, _) = dwt::forward(&y_p);
    let coeffs = dct::forward_image(&ll);
    let (ll_h, ll_w) = ll.dim();
    let mut bits = Vec::with_capacity(total_bits);
    'outer: for by in 0..ll_h / 8 {
        for bx in 0..ll_w / 8 {
            for &(u, v) in &dct::MIDBAND {
                if bits.len() >= total_bits {
                    break 'outer;
                }
                bits.push(qim::extract(coeffs[[by * 8 + u, bx * 8 + v]], delta));
            }
        }
    }
    if bits.len() < total_bits {
        return Err(format!(
            "image too small for {total_bits} bits (got {})",
            bits.len()
        ));
    }
    Ok((bits, det))
}

fn psnr_y(a: &Array2<f32>, b: &Array2<f32>) -> f32 {
    let mse = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        / a.len() as f32;
    if mse <= 1e-9 {
        return 99.0;
    }
    10.0 * ((255.0 * 255.0 / mse).log10())
}

fn sibling_path(out: &str, tag: &str) -> String {
    let p = std::path::Path::new(out);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("png");
    let parent = p.parent().and_then(|s| s.to_str()).unwrap_or(".");
    format!("{parent}/{stem}.{tag}.{ext}")
}
