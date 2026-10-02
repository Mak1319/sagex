use crate::central::CentralDirectoryFileHeader;
use crate::error::{Error, OL2WMResult};
use crate::extra::ExtraFields;
use crate::flags::{CompressionMethod, DosDate, DosTime, GeneralPurposeBitFlags};
use crate::local::LocalFileHeader;

pub(crate) const ALLOWED: [usize; 8] = [4, 8, 16, 32, 64, 128, 256, 512];
pub(crate) const MARKERS: [u32; 4] = [0x200B, 0x200C, 0x200D, 0x2060];
pub const CUSTOM_REL_TYPE: &str = "http://sagex/wm/relationships/watermark";
pub const FONT_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/font";

pub(crate) fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

pub(crate) fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

pub(crate) fn hash16(b: &[u8]) -> u16 {
    let mut h: u32 = 0x811c9dc5;
    for &x in b {
        h ^= x as u32;
        h = h.wrapping_mul(0x01000193);
    }
    (h & 0xffff) as u16
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    OpcDocx,
    OpcXlsx,
    OpcPptx,
    OpcGeneric,
    Odf,
    Generic,
}

impl ContainerKind {
    pub fn detect(parts: &[String]) -> Self {
        let has = |n: &str| parts.iter().any(|p| p == n);
        let any_prefix = |pre: &str| parts.iter().any(|p| p.starts_with(pre));
        if has("[Content_Types].xml") {
            if any_prefix("word/") {
                return Self::OpcDocx;
            }
            if any_prefix("xl/") {
                return Self::OpcXlsx;
            }
            if any_prefix("ppt/") {
                return Self::OpcPptx;
            }
            return Self::OpcGeneric;
        }
        if has("mimetype") && has("content.xml") {
            return Self::Odf;
        }
        Self::Generic
    }
}

fn be16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}
fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}
fn sbe16(v: i16) -> [u8; 2] {
    v.to_be_bytes()
}

fn ttf_checksum(data: &[u8]) -> u32 {
    let mut s: u32 = 0;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        s = s.wrapping_add(u32::from_be_bytes([c[0], c[1], c[2], c[3]]));
    }
    let r = chunks.remainder();
    if !r.is_empty() {
        let mut last = [0u8; 4];
        last[..r.len()].copy_from_slice(r);
        s = s.wrapping_add(u32::from_be_bytes(last));
    }
    s
}

/// Encode raw bytes as one simple-glyph outline: xs run then ys run,
/// matching TrueType layout. Each point carries 4 payload bytes
/// (x i16 + y i16, on-curve). Zero-area micro-path.
fn data_glyph(xs_raw: &[u8], ys_raw: &[u8]) -> Vec<u8> {
    assert_eq!(xs_raw.len(), ys_raw.len());
    assert!(xs_raw.len() % 2 == 0);
    let n = xs_raw.len() / 2;
    let mut xs: Vec<i16> = Vec::with_capacity(n);
    let mut ys: Vec<i16> = Vec::with_capacity(n);
    for q in xs_raw.chunks_exact(2) {
        xs.push(i16::from_le_bytes([q[0], q[1]]));
    }
    for q in ys_raw.chunks_exact(2) {
        ys.push(i16::from_le_bytes([q[0], q[1]]));
    }
    // deltas from (0,0): first point absolute, rest relative
    let mut dx = Vec::with_capacity(n);
    let mut dy = Vec::with_capacity(n);
    let (mut px, mut py) = (0i32, 0i32);
    let (mut xmin, mut ymin, mut xmax, mut ymax) = (0i32, 0i32, 0i32, 0i32);
    for (x, y) in xs.iter().zip(ys.iter()) {
        dx.push((*x as i32 - px) as i16);
        dy.push((*y as i32 - py) as i16);
        px = *x as i32;
        py = *y as i32;
        xmin = xmin.min(px);
        ymin = ymin.min(py);
        xmax = xmax.max(px);
        ymax = ymax.max(py);
    }
    let mut g = Vec::new();
    g.extend_from_slice(&sbe16(1)); // one contour
    g.extend_from_slice(&sbe16(xmin as i16));
    g.extend_from_slice(&sbe16(ymin as i16));
    g.extend_from_slice(&sbe16(xmax as i16));
    g.extend_from_slice(&sbe16(ymax as i16));
    g.extend_from_slice(&be16((n - 1) as u16)); // endPts
    g.extend_from_slice(&be16(0)); // no instructions
    g.extend(std::iter::repeat_n(0x01u8, n)); // all on-curve, long deltas
    for d in &dx {
        g.extend_from_slice(&sbe16(*d));
    }
    for d in &dy {
        g.extend_from_slice(&sbe16(*d));
    }
    g
}

/// Minimal 5-glyph TTF: .notdef + 4 data glyphs for MARKERS.
/// Payload layout: len u16le + crc16 u16le + payload, striped over glyphs.
fn synth_ttf(face: &str, payload: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    raw.extend_from_slice(&crc16(payload).to_le_bytes());
    raw.extend_from_slice(payload);
    while raw.len() % 16 != 0 {
        raw.push(0);
    }
    let per = raw.len() / 4;
    let mut glyph_vecs: Vec<Vec<u8>> = Vec::new();
    for q in raw.chunks_exact(per) {
        let (xs, ys) = q.split_at(per / 2);
        glyph_vecs.push(data_glyph(xs, ys));
    }
    // loca short format needs even offsets
    for g in glyph_vecs.iter_mut() {
        if g.len() % 2 != 0 {
            g.push(0);
        }
    }
    let glyphs = glyph_vecs;

    let mut head = Vec::new();
    head.extend_from_slice(&be32(0x00010000));
    head.extend_from_slice(&be32(0));
    head.extend_from_slice(&be32(0));
    head.extend_from_slice(&be32(0x5F0F3CF5));
    head.extend_from_slice(&be16(0x0003));
    head.extend_from_slice(&be16(1000));
    head.extend_from_slice(&[0u8; 16]);
    for _ in 0..4 {
        head.extend_from_slice(&sbe16(0));
    }
    head.extend_from_slice(&be16(0));
    head.extend_from_slice(&be16(8));
    head.extend_from_slice(&sbe16(2));
    head.extend_from_slice(&sbe16(0));
    head.extend_from_slice(&sbe16(0));

    let mut hhea = Vec::new();
    hhea.extend_from_slice(&be32(0x00010000));
    hhea.extend_from_slice(&sbe16(800));
    hhea.extend_from_slice(&sbe16(-200));
    hhea.extend_from_slice(&sbe16(200));
    hhea.extend_from_slice(&be16(500));
    hhea.extend_from_slice(&sbe16(0));
    hhea.extend_from_slice(&sbe16(0));
    hhea.extend_from_slice(&sbe16(0));
    hhea.extend_from_slice(&sbe16(1));
    hhea.extend_from_slice(&sbe16(0));
    hhea.extend_from_slice(&sbe16(0));
    for _ in 0..4 {
        hhea.extend_from_slice(&sbe16(0));
    }
    hhea.extend_from_slice(&sbe16(0));
    hhea.extend_from_slice(&be16(5));

    let mut maxp = vec![0u8; 32];
    maxp[0..4].copy_from_slice(&be32(0x00010000));
    maxp[4..6].copy_from_slice(&be16(5));

    let mut os2 = Vec::new();
    os2.extend_from_slice(&be16(0));
    os2.extend_from_slice(&be16(500));
    os2.extend_from_slice(&be16(400));
    os2.extend_from_slice(&be16(5));
    os2.extend_from_slice(&be16(0));
    for v in [650u16, 700, 0, 140, 650, 700, 0, 479, 49] {
        os2.extend_from_slice(&be16(v));
    }
    os2.extend_from_slice(&sbe16(258));
    os2.extend_from_slice(&be16(0));
    os2.extend_from_slice(&[0u8; 10]);
    os2.extend_from_slice(&be32(0));
    os2.extend_from_slice(&be32(1 << 19));
    os2.extend_from_slice(&be32(0));
    os2.extend_from_slice(&be32(0));
    os2.extend_from_slice(b"SGWX");
    os2.extend_from_slice(&be16(0x40));
    os2.extend_from_slice(&be16(0x200B));
    os2.extend_from_slice(&be16(0x2060));
    os2.extend_from_slice(&sbe16(800));
    os2.extend_from_slice(&sbe16(-200));
    os2.extend_from_slice(&sbe16(200));
    os2.extend_from_slice(&be16(800));
    os2.extend_from_slice(&be16(200));
    os2.extend_from_slice(&be32(0));
    os2.extend_from_slice(&be32(0));

    let fam = face.encode_utf16().collect::<Vec<u16>>();
    let sub: Vec<u16> = "Regular".encode_utf16().collect();
    let mut name = Vec::new();
    name.extend_from_slice(&be16(0));
    name.extend_from_slice(&be16(2));
    name.extend_from_slice(&be16(30));
    let mut off = 0u16;
    for (id, s) in [(1u16, &fam), (2u16, &sub)] {
        name.extend_from_slice(&be16(3));
        name.extend_from_slice(&be16(1));
        name.extend_from_slice(&be16(0x409));
        name.extend_from_slice(&be16(id));
        name.extend_from_slice(&be16((s.len() * 2) as u16));
        name.extend_from_slice(&be16(off));
        off += (s.len() * 2) as u16;
    }
    for s in [&fam, &sub] {
        for u in s.iter() {
            name.extend_from_slice(&be16(*u));
        }
    }

    // cmap format 4: 4 segments + sentinel
    let ends = [0x200Bu16, 0x200C, 0x200D, 0x2060, 0xFFFF];
    let mut f4 = Vec::new();
    f4.extend_from_slice(&be16(4));
    f4.extend_from_slice(&be16(56));
    f4.extend_from_slice(&be16(0));
    f4.extend_from_slice(&be16(10));
    f4.extend_from_slice(&be16(8));
    f4.extend_from_slice(&be16(2));
    f4.extend_from_slice(&be16(2));
    for e in ends {
        f4.extend_from_slice(&be16(e));
    }
    f4.extend_from_slice(&be16(0));
    for s in ends {
        f4.extend_from_slice(&be16(s));
    }
    for (i, c) in [0x200Bu16, 0x200C, 0x200D, 0x2060].iter().enumerate() {
        f4.extend_from_slice(&be16((i as u16 + 1).wrapping_sub(*c)));
    }
    f4.extend_from_slice(&be16(1));
    for _ in 0..5 {
        f4.extend_from_slice(&be16(0));
    }
    let mut cmap = Vec::new();
    cmap.extend_from_slice(&be16(0));
    cmap.extend_from_slice(&be16(1));
    cmap.extend_from_slice(&be16(3));
    cmap.extend_from_slice(&be16(1));
    cmap.extend_from_slice(&be32(12));
    cmap.extend_from_slice(&f4);

    let mut post = vec![0u8; 32];
    post[0..4].copy_from_slice(&be32(0x00030000));

    let mut hmtx = Vec::new();
    for _ in 0..5 {
        hmtx.extend_from_slice(&be16(500));
        hmtx.extend_from_slice(&sbe16(0));
    }
    // loca short: .notdef at 0, then data glyphs
    let mut loca_vals = vec![0u32];
    let mut acc = 10u32; // .notdef is 10 zero bytes
    loca_vals.push(acc / 2);
    for g in &glyphs {
        acc += g.len() as u32;
        loca_vals.push(acc / 2);
    }
    let mut loca = Vec::new();
    for v in loca_vals {
        loca.extend_from_slice(&be16(v as u16));
    }
    let mut glyf = vec![0u8; 10];
    for g in &glyphs {
        glyf.extend_from_slice(g);
    }

    let mut tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"OS/2", os2),
        (b"cmap", cmap),
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"maxp", maxp),
        (b"name", name),
        (b"post", post),
    ];
    tables.sort_by(|a, b| a.0.cmp(b.0));
    let n = tables.len() as u32;
    let mut out = Vec::new();
    out.extend_from_slice(&be32(0x00010000));
    out.extend_from_slice(&be16(n as u16));
    out.extend_from_slice(&be16(128));
    out.extend_from_slice(&be16(3));
    out.extend_from_slice(&be16(32));
    let mut offset = 12 + (n as usize) * 16;
    for (tag, data) in &tables {
        out.extend_from_slice(*tag);
        out.extend_from_slice(&be32(ttf_checksum(data)));
        out.extend_from_slice(&be32(offset as u32));
        out.extend_from_slice(&be32(data.len() as u32));
        offset += (data.len() + 3) / 4 * 4;
    }
    for (_, data) in &tables {
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    let sum = ttf_checksum(&out);
    let adj = 0xB1B0AFBAu32.wrapping_sub(sum);
    let head_idx = tables.iter().position(|(t, _)| *t == b"head").unwrap();
    let data_off =
        u32::from_be_bytes(out[12 + head_idx * 16 + 8..12 + head_idx * 16 + 12].try_into().unwrap())
            as usize;
    out[data_off + 8..data_off + 12].copy_from_slice(&be32(adj));
    out
}

fn odttf_obfuscate(ttf: &[u8], guid16: &[u8; 16]) -> Vec<u8> {
    let mut rev = *guid16;
    rev.reverse();
    let mut out = ttf.to_vec();
    for i in 0..32.min(out.len()) {
        out[i] ^= rev[i % 16];
    }
    out
}

fn random_guid() -> ([u8; 16], String) {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs().wrapping_mul(0x9e3779b1))
        .unwrap_or(0x243f6a88);
    let mut s = t | 1;
    let mut b = [0u8; 16];
    for x in b.iter_mut() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        *x = (s >> 11) as u8;
    }
    let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
    let g = format!(
        "{{{}-{}-{}-{}-{}}}",
        &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]
    );
    (b, g)
}

fn guid_to_bytes(g: &str) -> OL2WMResult<[u8; 16]> {
    let hex: String = g.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return Err(Error::BadCover { detail: "bad fontKey GUID" });
    }
    let mut b = [0u8; 16];
    for i in 0..16 {
        b[i] = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .map_err(|_| Error::BadCover { detail: "bad fontKey GUID" })?;
    }
    Ok(b)
}

fn table_dir(font: &[u8], tag: &[u8; 4]) -> OL2WMResult<(usize, usize)> {
    if font.len() < 12 {
        return Err(Error::BadCover { detail: "font too small" });
    }
    let n = u16::from_be_bytes([font[4], font[5]]) as usize;
    if font.len() < 12 + n * 16 {
        return Err(Error::BadCover { detail: "bad table directory" });
    }
    for i in 0..n {
        let o = 12 + i * 16;
        if &font[o..o + 4] == tag {
            let off = u32::from_be_bytes(font[o + 8..o + 12].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(font[o + 12..o + 16].try_into().unwrap()) as usize;
            if off + len > font.len() {
                return Err(Error::BadCover { detail: "table out of range" });
            }
            return Ok((off, len));
        }
    }
    Err(Error::NoWatermark)
}

/// De-obfuscate first 32 bytes from known plaintext (10-table layout).
fn deobfuscate_self(font: &[u8]) -> Option<Vec<u8>> {
    if font.len() < 44 {
        return None;
    }
    // known plaintext: sfnt, numTables=10, searchRange/selector/shift, 'OS/2'
    let mut plain = [0u8; 16];
    plain[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]);
    plain[4..6].copy_from_slice(&[0x00, 0x0A]);
    plain[6..8].copy_from_slice(&[0x00, 0x80]);
    plain[8..10].copy_from_slice(&[0x00, 0x03]);
    plain[10..12].copy_from_slice(&[0x00, 0x20]);
    plain[12..16].copy_from_slice(b"OS/2");
    // bytes 24..28 are OS/2 length: standard synth length
    let mut rev = [0u8; 16];
    for i in 0..16 {
        rev[i] = font[i] ^ plain[i];
    }
    // verify: bytes 28..32 must decode to 'cmap'
    let tag: Vec<u8> = (28..32).map(|i| font[i] ^ rev[i % 16]).collect();
    if tag != b"cmap" {
        // fallback: OS/2 length may differ after subsetting; scan rev candidates is
        // infeasible — require fontKey instead
        return None;
    }
    let mut out = font.to_vec();
    for i in 0..32.min(out.len()) {
        out[i] ^= rev[i % 16];
    }
    Some(out)
}

/// Format-4 cmap lookup for one char.
fn cmap4_lookup(cmap: &[u8], sub_off: usize, ch: u32) -> Option<u16> {
    let seg_x2 = u16::from_be_bytes([cmap[sub_off + 6], cmap[sub_off + 7]]) as usize / 2;
    let ends = sub_off + 14;
    let starts = ends + 2 * seg_x2 + 2;
    let deltas = starts + 2 * seg_x2;
    let ranges = deltas + 2 * seg_x2;
    for i in 0..seg_x2 {
        let end = u16::from_be_bytes([cmap[ends + 2 * i], cmap[ends + 2 * i + 1]]) as u32;
        let start = u16::from_be_bytes([cmap[starts + 2 * i], cmap[starts + 2 * i + 1]]) as u32;
        if ch < start || ch > end {
            continue;
        }
        let ro = u16::from_be_bytes([cmap[ranges + 2 * i], cmap[ranges + 2 * i + 1]]);
        if ro == 0 {
            let d = i16::from_be_bytes([cmap[deltas + 2 * i], cmap[deltas + 2 * i + 1]]);
            return Some(ch.wrapping_add(d as u32) as u16);
        }
        return None; // range-offset form not emitted by synth; subset keeps deltas
    }
    None
}

/// Decode one simple glyph's payload bytes; supports short+long coordinate forms.
fn glyph_bytes(glyf: &[u8], off: usize, len: usize) -> OL2WMResult<Vec<u8>> {
    if len < 12 {
        return Err(Error::BadCover { detail: "glyph too small" });
    }
    let n_cont = i16::from_be_bytes([glyf[off], glyf[off + 1]]);
    if n_cont != 1 {
        return Err(Error::BadCover { detail: "data glyph must have 1 contour" });
    }
    let n_pts = u16::from_be_bytes([glyf[off + 10], glyf[off + 11]]) as usize + 1;
    let mut p = off + 12;
    // instructionLength
    let ilen = u16::from_be_bytes([glyf[p], glyf[p + 1]]) as usize;
    p += 2 + ilen;
    // flags (with repeat support)
    let mut flags: Vec<u8> = Vec::with_capacity(n_pts);
    while flags.len() < n_pts {
        if p >= off + len {
            return Err(Error::BadCover { detail: "flags overrun" });
        }
        let f = glyf[p];
        p += 1;
        flags.push(f);
        if f & 0x08 != 0 {
            if p >= off + len {
                return Err(Error::BadCover { detail: "repeat overrun" });
            }
            let r = glyf[p] as usize;
            p += 1;
            for _ in 0..r {
                flags.push(f);
            }
        }
    }
    if flags.len() != n_pts {
        return Err(Error::BadCover { detail: "flag count mismatch" });
    }
    let mut out = Vec::with_capacity(n_pts * 4);
    for axis in 0..2 {
        let mut v = 0i32;
        for f in &flags {
            let d: i16;
            if axis == 0 {
                if f & 0x02 != 0 {
                    if p >= off + len {
                        return Err(Error::BadCover { detail: "x overrun" });
                    }
                    let b = glyf[p] as i16;
                    p += 1;
                    d = if f & 0x10 != 0 { b } else { -b };
                } else {
                    if f & 0x10 != 0 {
                        return Err(Error::BadCover { detail: "x missing" });
                    }
                    if p + 2 > off + len {
                        return Err(Error::BadCover { detail: "x overrun" });
                    }
                    d = i16::from_be_bytes([glyf[p], glyf[p + 1]]);
                    p += 2;
                }
                v += d as i32;
                out.extend_from_slice(&(v as i16).to_le_bytes());
            } else {
                if f & 0x04 != 0 {
                    if p >= off + len {
                        return Err(Error::BadCover { detail: "y overrun" });
                    }
                    let b = glyf[p] as i16;
                    p += 1;
                    d = if f & 0x20 != 0 { b } else { -b };
                } else {
                    if f & 0x20 != 0 {
                        return Err(Error::BadCover { detail: "y missing" });
                    }
                    if p + 2 > off + len {
                        return Err(Error::BadCover { detail: "y overrun" });
                    }
                    d = i16::from_be_bytes([glyf[p], glyf[p + 1]]);
                    p += 2;
                }
                v += d as i32;
                out.extend_from_slice(&(v as i16).to_le_bytes());
            }
        }
    }
    Ok(out)
}

fn zero_flags() -> GeneralPurposeBitFlags {
    GeneralPurposeBitFlags::from_bytes([0, 0])
}

#[derive(Debug, Clone)]
pub struct FontWatermark {
    pub path: String,
    pub face: String,
    pub font_key: String,
    pub font_bytes: Vec<u8>,
    pub payload_len: usize,
    pub flavor: ContainerKind,
}

impl FontWatermark {
    pub fn new(payload: Vec<u8>, flavor: ContainerKind) -> OL2WMResult<Self> {
        if !ALLOWED.contains(&payload.len()) {
            return Err(Error::BadSize { got: payload.len() });
        }
        let h = hash16(&payload);
        let face = format!("WmFace{h:04}");
        let ttf = synth_ttf(&face, &payload);
        let (guid, key) = random_guid();
        let font_bytes = odttf_obfuscate(&ttf, &guid);
        let dir = match flavor {
            ContainerKind::OpcDocx => "word/fonts",
            ContainerKind::OpcXlsx => "xl/fonts",
            ContainerKind::OpcPptx => "ppt/fonts",
            _ => "fonts",
        };
        Ok(Self {
            path: format!("{dir}/wm{h:04}.odttf"),
            face,
            font_key: key,
            font_bytes,
            payload_len: payload.len(),
            flavor,
        })
    }

    pub fn to_local(&self) -> LocalFileHeader {
        LocalFileHeader {
            version: 10,
            flags: zero_flags(),
            compression_method: CompressionMethod::None,
            mod_time: DosTime { raw: 0 },
            mod_date: DosDate { raw: 0x21 },
            crc32: crc32(&self.font_bytes),
            compressed_size: self.font_bytes.len() as u32,
            uncompressed_size: self.font_bytes.len() as u32,
            file_name: self.path.as_bytes().to_vec(),
            extra: ExtraFields::default(),
            data: self.font_bytes.clone(),
        }
    }

    pub fn font_central_for(l: &LocalFileHeader, lfh_offset: u32) -> CentralDirectoryFileHeader {
        let mut c = CentralDirectoryFileHeader {
            version_made: 10,
            version_extract: 10,
            flags: l.flags,
            compression_method: l.compression_method,
            mod_time: l.mod_time,
            mod_date: l.mod_date,
            crc32: l.crc32,
            compressed_size: l.compressed_size,
            uncompressed_size: l.uncompressed_size,
            file_name: l.file_name.clone(),
            extra: l.extra.clone(),
            comment: Vec::new(),
            disk_number: 0,
            internal_attrs: 0,
            external_attrs: 0x20,
            local_header_offset: lfh_offset,
        };
        c.external_attrs = 0x20;
        c
    }

    pub fn media_rel_id(&self) -> String {
        format!("rWm{}m", &self.face[6..])
    }

    pub fn custom_rel_id(&self) -> String {
        format!("rWm{}c", &self.face[6..])
    }

    fn candidates(
        font_key: Option<&str>,
        font_bytes: &[u8],
    ) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if let Some(k) = font_key {
            if let Ok(g) = guid_to_bytes(k) {
                out.push(odttf_obfuscate(font_bytes, &g));
            }
        }
        if let Some(f) = deobfuscate_self(font_bytes) {
            out.push(f);
        }
        out.push(font_bytes.to_vec());
        out
    }

    /// Extract payload from glyph outlines. Tries fontKey, self-recovery, raw.
    pub fn payload_from_font(font_key: Option<&str>, font_bytes: &[u8]) -> OL2WMResult<Vec<u8>> {
        let mut any_table = false;
        for font in Self::candidates(font_key, font_bytes).iter() {
            let (glyf_off, glyf_len) = match table_dir(font, b"glyf") {
                Ok(t) => {
                    any_table = true;
                    t
                }
                Err(_) => continue,
            };
            let (loca_off, _) = match table_dir(font, b"loca") {
                Ok(t) => t,
                Err(_) => continue,
            };
            let (maxp_off, _) = match table_dir(font, b"maxp") {
                Ok(t) => t,
                Err(_) => continue,
            };
            let (head_off, _) = match table_dir(font, b"head") {
                Ok(t) => t,
                Err(_) => continue,
            };
            let n_glyphs = u16::from_be_bytes([font[maxp_off + 4], font[maxp_off + 5]]) as usize;
            let long_loca = i16::from_be_bytes([font[head_off + 50], font[head_off + 51]]) != 0;
            // glyph ids for markers via cmap (fall back to 1..4)
            let mut gids: Vec<usize> = vec![1, 2, 3, 4];
            if let Ok((cmap_off, _)) = table_dir(font, b"cmap") {
                let n_sub = u16::from_be_bytes([font[cmap_off + 2], font[cmap_off + 3]]) as usize;
                let mut found = Vec::new();
                for i in 0..n_sub {
                    let o = cmap_off + 4 + i * 8;
                    if o + 8 > font.len() {
                        break;
                    }
                    let fmt = u16::from_be_bytes([font[cmap_off + u32::from_be_bytes([font[o + 4], font[o + 5], font[o + 6], font[o + 7]]) as usize], font[cmap_off + u32::from_be_bytes([font[o + 4], font[o + 5], font[o + 6], font[o + 7]]) as usize] + 1]);
                    let sub = cmap_off + u32::from_be_bytes([font[o + 4], font[o + 5], font[o + 6], font[o + 7]]) as usize;
                    if sub + 8 > font.len() || fmt != 4 {
                        continue;
                    }
                    let mut ids = Vec::new();
                    let mut ok = true;
                    for ch in MARKERS {
                        match cmap4_lookup(&font, sub, ch) {
                            Some(g) => ids.push(g as usize),
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        found = ids;
                        break;
                    }
                }
                if !found.is_empty() {
                    gids = found;
                }
            }
            // glyph spans via loca
            let at = |idx: usize| -> Option<usize> {
                if long_loca {
                    if loca_off + 4 * idx + 4 > font.len() {
                        return None;
                    }
                    Some(u32::from_be_bytes([
                        font[loca_off + 4 * idx],
                        font[loca_off + 4 * idx + 1],
                        font[loca_off + 4 * idx + 2],
                        font[loca_off + 4 * idx + 3],
                    ]) as usize)
                } else {
                    if loca_off + 2 * idx + 2 > font.len() {
                        return None;
                    }
                    Some(
                        u16::from_be_bytes([font[loca_off + 2 * idx], font[loca_off + 2 * idx + 1]])
                            as usize
                            * 2,
                    )
                }
            };
            let mut cat = Vec::new();
            let mut ok = true;
            for gid in &gids {
                if *gid >= n_glyphs {
                    ok = false;
                    break;
                }
                let (s, e) = match (at(*gid), at(gid + 1)) {
                    (Some(s), Some(e)) => (s, e),
                    _ => {
                        ok = false;
                        break;
                    }
                };
                if s == e || glyf_off + e > glyf_off + glyf_len {
                    ok = false;
                    break;
                }
                match glyph_bytes(font, glyf_off + s, e - s) {
                    Ok(b) => cat.extend_from_slice(&b),
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok || cat.len() < 4 {
                continue;
            }
            let len = u16::from_le_bytes([cat[0], cat[1]]) as usize;
            let crc = u16::from_le_bytes([cat[2], cat[3]]);
            if !ALLOWED.contains(&len) || cat.len() < 4 + len {
                continue;
            }
            let payload = cat[4..4 + len].to_vec();
            if crc16(&payload) != crc {
                continue;
            }
            return Ok(payload);
        }
        if any_table {
            Err(Error::BadCover { detail: "glyph payload missing or corrupt" })
        } else {
            Err(Error::NoWatermark)
        }
    }
}
