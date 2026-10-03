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

    fn name_string(font: &[u8], name_id: u16) -> Option<String> {
        let (off, len) = table_dir(font, b"name").ok()?;
        if len < 6 || off + 6 > font.len() {
            return None;
        }
        let count = u16::from_be_bytes([font[off + 2], font[off + 3]]) as usize;
        let storage = off + u16::from_be_bytes([font[off + 4], font[off + 5]]) as usize;
        let mut best: Option<String> = None;
        for i in 0..count {
            let r = off + 6 + i * 12;
            if r + 12 > off + len || r + 12 > font.len() {
                break;
            }
            let platform = u16::from_be_bytes([font[r], font[r + 1]]);
            let enc = u16::from_be_bytes([font[r + 2], font[r + 3]]);
            let id = u16::from_be_bytes([font[r + 6], font[r + 7]]);
            if id != name_id {
                continue;
            }
            let slen = u16::from_be_bytes([font[r + 8], font[r + 9]]) as usize;
            let soff = u16::from_be_bytes([font[r + 10], font[r + 11]]) as usize;
            if storage + soff + slen > font.len() {
                continue;
            }
            let raw = &font[storage + soff..storage + soff + slen];
            let s = if platform == 3 || (platform == 0) {
                if raw.len() % 2 != 0 {
                    continue;
                }
                String::from_utf16(
                    &raw.chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]]))
                        .collect::<Vec<_>>(),
                )
                .ok()?
            } else {
                String::from_utf8_lossy(raw).into_owned()
            };
            // prefer windows unicode english
            if platform == 3 && enc == 1 {
                return Some(s);
            }
            if best.is_none() {
                best = Some(s);
            }
        }
        best
    }

    /// Current family name from the font's own name table.
    pub fn family_name(font_key: Option<&str>, font_bytes: &[u8]) -> Option<String> {
        for font in Self::candidates(font_key, font_bytes) {
            if let Some(n) = Self::name_string(&font, 1) {
                return Some(n);
            }
        }
        None
    }

    /// Strip subset uniformity prefix: "ABCDEF+Face" -> "Face".
    pub fn strip_subset_prefix(face: &str) -> &str {
        if let Some((pre, rest)) = face.split_once('+') {
            if pre.len() == 6 && pre.chars().all(|c| c.is_ascii_alphanumeric()) {
                return rest;
            }
        }
        face
    }

    /// Fast pre-filter: candidate parses as a font with our marker codepoints.
    pub fn has_markers(font_key: Option<&str>, font_bytes: &[u8]) -> bool {
        for font in Self::candidates(font_key, font_bytes) {
            let (cmap_off, _) = match table_dir(&font, b"cmap") {
                Ok(t) => t,
                Err(_) => continue,
            };
            if cmap_off + 4 > font.len() {
                continue;
            }
            let n_sub = u16::from_be_bytes([font[cmap_off + 2], font[cmap_off + 3]]) as usize;
            for i in 0..n_sub {
                let o = cmap_off + 4 + i * 8;
                if o + 8 > font.len() {
                    break;
                }
                let sub = cmap_off
                    + u32::from_be_bytes([font[o + 4], font[o + 5], font[o + 6], font[o + 7]])
                        as usize;
                if sub + 8 > font.len() {
                    continue;
                }
                if u16::from_be_bytes([font[sub], font[sub + 1]]) != 4 {
                    continue;
                }
                let mut all = true;
                for ch in MARKERS {
                    if cmap4_lookup(&font, sub, ch).is_none() {
                        all = false;
                        break;
                    }
                }
                if all {
                    return true;
                }
            }
        }
        false
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

// ---------- directory channel ----------

const B32: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
const COMPS_PER_ENTRY: usize = 6;
const COMP_LEN: usize = 8;

pub(crate) fn b32_encode(b: &[u8]) -> String {
    let mut out = String::new();
    let mut buf: u32 = 0;
    let mut bits = 0;
    for &x in b {
        buf = (buf << 8) | x as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(B32[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn b32_decode(s: &str) -> OL2WMResult<Vec<u8>> {
    let mut buf: u32 = 0;
    let mut bits = 0;
    let mut out = Vec::new();
    for ch in s.chars() {
        let v = B32.iter().position(|&c| c as char == ch).ok_or(Error::BadCover {
            detail: "bad base32 char in path",
        })? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub data: Vec<u8>,
    /// Unindexed trailing bytes after compressed data; len == data len.
    pub trailer: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DirWatermark {
    pub entries: Vec<DirEntry>,
}

impl DirWatermark {
    pub fn new(payload: Vec<u8>) -> OL2WMResult<Self> {
        if !ALLOWED.contains(&payload.len()) {
            return Err(Error::BadSize { got: payload.len() });
        }
        let b32 = b32_encode(&payload);
        let mut comps: Vec<String> = b32
            .as_bytes()
            .chunks(COMP_LEN)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect();
        if comps.is_empty() {
            comps.push("a".into());
        }
        let prefix = format!("w{:04}", hash16(&payload));
        let n_entries = comps.len().div_ceil(COMPS_PER_ENTRY).max(1);
        let data_chunk = payload.len().div_ceil(n_entries).max(1);
        let mut entries = Vec::new();
        for (i, cc) in comps.chunks(COMPS_PER_ENTRY).enumerate() {
            let d0 = (i * data_chunk).min(payload.len());
            let d1 = ((i + 1) * data_chunk).min(payload.len());
            let data = payload[d0..d1].to_vec();
            let trailer = Self::trailer_bytes(&data, i);
            let name = format!("{prefix}/{i:02}/{}/", cc.join("/"));
            entries.push(DirEntry { name, data, trailer });
        }
        Ok(Self { entries })
    }

    fn trailer_bytes(data: &[u8], salt: usize) -> Vec<u8> {
        let mut s = hash16(data).wrapping_add((salt as u16).wrapping_mul(0x9e37)) as u64 | 1;
        s ^= (data.len() as u64).wrapping_mul(0x85ebca6b);
        data.iter()
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s >> 11) as u8
            })
            .collect()
    }

    pub fn decode_names(names: &[String]) -> OL2WMResult<Vec<u8>> {
        let mut sorted = names.to_vec();
        sorted.sort();
        let mut cat = String::new();
        for n in &sorted {
            let parts: Vec<&str> = n.split('/').filter(|s| !s.is_empty()).collect();
            if parts.len() < 3 {
                continue;
            }
            for p in &parts[2..] {
                cat.push_str(p);
            }
        }
        b32_decode(&cat)
    }

    pub fn entry_to_local(e: &DirEntry) -> LocalFileHeader {
        LocalFileHeader {
            version: 10,
            flags: zero_flags(),
            compression_method: CompressionMethod::None,
            mod_time: DosTime { raw: 0 },
            mod_date: DosDate { raw: 0x21 },
            crc32: crc32(&e.data),
            compressed_size: e.data.len() as u32,
            uncompressed_size: e.data.len() as u32,
            file_name: e.name.as_bytes().to_vec(),
            extra: ExtraFields::default(),
            data: e.data.clone(),
        }
    }

    fn entry_to_central(e: &DirEntry, lfh_offset: u32) -> CentralDirectoryFileHeader {
        CentralDirectoryFileHeader {
            version_made: 10,
            version_extract: 10,
            flags: zero_flags(),
            compression_method: CompressionMethod::None,
            mod_time: DosTime { raw: 0 },
            mod_date: DosDate { raw: 0x21 },
            crc32: crc32(&e.data),
            compressed_size: e.data.len() as u32,
            uncompressed_size: e.data.len() as u32,
            file_name: e.name.as_bytes().to_vec(),
            extra: ExtraFields::default(),
            comment: Vec::new(),
            disk_number: 0,
            internal_attrs: 0,
            external_attrs: 0x41ED0010,
            local_header_offset: lfh_offset,
        }
    }

    pub fn to_locals(&self) -> Vec<LocalFileHeader> {
        self.entries.iter().map(Self::entry_to_local).collect()
    }

    pub fn to_centrals(&self, offsets: &[u32]) -> Vec<CentralDirectoryFileHeader> {
        self.entries
            .iter()
            .zip(offsets.iter())
            .map(|(e, &o)| Self::entry_to_central(e, o))
            .collect()
    }

    pub fn entry_central_for(
        l: &LocalFileHeader,
        lfh_offset: u32,
    ) -> CentralDirectoryFileHeader {
        CentralDirectoryFileHeader {
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
            external_attrs: 0x41ED0010,
            local_header_offset: lfh_offset,
        }
    }
}

// ---------- file channel (media covers) ----------

fn png_chunk(ty: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[ty.as_slice(), data].concat()).to_be_bytes());
    out
}

fn png_wrap(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let ihdr = vec![0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0];
    out.extend(png_chunk(b"IHDR", &ihdr));
    out.extend(png_chunk(b"wmWM", payload));
    out.extend(png_chunk(b"IEND", &[]));
    out
}

fn wav_wrap(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + payload.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&8000u32.to_le_bytes());
    out.extend_from_slice(&8000u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn png_unwrap(file: &[u8]) -> OL2WMResult<Vec<u8>> {
    if file.len() < 8 || file[..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err(Error::BadCover { detail: "not a PNG" });
    }
    let mut p = 8;
    while p + 8 <= file.len() {
        let len =
            u32::from_be_bytes(file[p..p + 4].try_into().map_err(|_| Error::BadCover {
                detail: "truncated PNG chunk",
            })?) as usize;
        let ty = &file[p + 4..p + 8];
        if ty == b"wmWM" {
            return Ok(file[p + 8..p + 8 + len.min(file.len() - p - 8)].to_vec());
        }
        p += 12 + len;
    }
    Err(Error::BadCover { detail: "wmWM chunk missing" })
}

fn wav_unwrap(file: &[u8]) -> OL2WMResult<Vec<u8>> {
    if file.len() < 48 || &file[..4] != b"RIFF" || &file[8..12] != b"WAVE" {
        return Err(Error::BadCover { detail: "not a WAV" });
    }
    let mut p = 12;
    while p + 8 <= file.len() {
        let id = &file[p..p + 4];
        let len =
            u32::from_le_bytes(file[p + 4..p + 8].try_into().map_err(|_| Error::BadCover {
                detail: "truncated WAV chunk",
            })?) as usize;
        if id == b"data" {
            return Ok(file[p + 8..p + 8 + len.min(file.len() - p - 8)].to_vec());
        }
        p += 8 + len;
    }
    Err(Error::BadCover { detail: "data chunk missing" })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverKind {
    Png,
    Wav,
}

impl CoverKind {
    pub fn parse(s: &str) -> OL2WMResult<Self> {
        match s {
            "png" => Ok(Self::Png),
            "wav" => Ok(Self::Wav),
            _ => Err(Error::BadCover { detail: "cover must be png or wav" }),
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Wav => "wav",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileWatermark {
    pub path: String,
    pub file_bytes: Vec<u8>,
    pub cover: CoverKind,
    pub payload_len: usize,
}

impl FileWatermark {
    pub fn new(payload: Vec<u8>, cover: CoverKind) -> OL2WMResult<Self> {
        if !ALLOWED.contains(&payload.len()) {
            return Err(Error::BadSize { got: payload.len() });
        }
        let b32 = b32_encode(&payload);
        let comps: Vec<String> = b32
            .as_bytes()
            .chunks(COMP_LEN)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect();
        let h = hash16(&payload);
        let groups: Vec<String> = comps
            .chunks(COMPS_PER_ENTRY)
            .enumerate()
            .map(|(i, cc)| format!("{i:02}_{}", cc.join("")))
            .collect();
        let path = format!("media/wm{h:04}/{}.{}", groups.join("/"), cover.ext());
        let file_bytes = match cover {
            CoverKind::Png => png_wrap(&payload),
            CoverKind::Wav => wav_wrap(&payload),
        };
        Ok(Self { path, file_bytes, cover, payload_len: payload.len() })
    }

    pub fn payload_from_file(path: &str, file_bytes: &[u8]) -> OL2WMResult<Vec<u8>> {
        if path.ends_with(".png") {
            png_unwrap(file_bytes)
        } else if path.ends_with(".wav") {
            wav_unwrap(file_bytes)
        } else {
            Err(Error::BadCover { detail: "unknown cover extension" })
        }
    }

    pub fn to_local(&self) -> LocalFileHeader {
        LocalFileHeader {
            version: 10,
            flags: zero_flags(),
            compression_method: CompressionMethod::None,
            mod_time: DosTime { raw: 0 },
            mod_date: DosDate { raw: 0x21 },
            crc32: crc32(&self.file_bytes),
            compressed_size: self.file_bytes.len() as u32,
            uncompressed_size: self.file_bytes.len() as u32,
            file_name: self.path.as_bytes().to_vec(),
            extra: ExtraFields::default(),
            data: self.file_bytes.clone(),
        }
    }

    pub fn file_central_for(l: &LocalFileHeader, lfh_offset: u32) -> CentralDirectoryFileHeader {
        CentralDirectoryFileHeader {
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
        }
    }
}

// ---------- linked channel (OPC content-type + rels) ----------

pub const IMAGE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const AUDIO_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio";

#[derive(Debug, Clone)]
pub struct LinkedWatermark {
    pub file: FileWatermark,
    pub media_rel_id: String,
    pub custom_rel_id: String,
}

impl LinkedWatermark {
    pub fn new(payload: Vec<u8>, cover: CoverKind) -> OL2WMResult<Self> {
        let file = FileWatermark::new(payload.clone(), cover)?;
        let h = hash16(&payload);
        Ok(Self {
            file,
            media_rel_id: format!("rWm{h:04}m"),
            custom_rel_id: format!("rWm{h:04}c"),
        })
    }

    pub fn content_type(&self) -> &'static str {
        match self.file.cover {
            CoverKind::Png => "image/png",
            CoverKind::Wav => "audio/wav",
        }
    }

    pub fn media_rel_type(&self) -> &'static str {
        match self.file.cover {
            CoverKind::Png => IMAGE_REL_TYPE,
            CoverKind::Wav => AUDIO_REL_TYPE,
        }
    }

    pub fn part_name(&self) -> String {
        format!("/{}", self.file.path)
    }

    pub fn patch_content_types(&self, xml: &[u8]) -> OL2WMResult<Vec<u8>> {
        if xml.windows(self.file.path.len()).any(|w| w == self.file.path.as_bytes()) {
            return Ok(xml.to_vec());
        }
        let element = format!(
            "<Override PartName=\"{}\" ContentType=\"{}\"/>",
            self.part_name(),
            self.content_type()
        );
        let out = insert_raw_before_end(xml, b"</Types>", &element)?;
        check_well_formed(&out)?;
        Ok(out)
    }

    pub fn patch_rels(&self, xml: &[u8]) -> OL2WMResult<Vec<u8>> {
        let part = self.part_name();
        let mut elements = String::new();
        if !xml.windows(self.media_rel_id.len()).any(|w| w == self.media_rel_id.as_bytes()) {
            elements.push_str(&format!(
                "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>",
                self.media_rel_id,
                self.media_rel_type(),
                part
            ));
        }
        if !xml.windows(self.custom_rel_id.len()).any(|w| w == self.custom_rel_id.as_bytes()) {
            elements.push_str(&format!(
                "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>",
                self.custom_rel_id, CUSTOM_REL_TYPE, part
            ));
        }
        if elements.is_empty() {
            return Ok(xml.to_vec());
        }
        insert_raw_before_end(xml, b"</Relationships>", &elements)
    }

    pub fn insert_override(xml: &[u8], element: &str) -> OL2WMResult<Vec<u8>> {
        insert_raw_before_end(xml, b"</Types>", element)
    }

    pub fn rels_path_for(parts: &[String]) -> (String, bool) {
        for c in [
            "word/_rels/document.xml.rels",
            "xl/_rels/workbook.xml.rels",
            "ppt/_rels/presentation.xml.rels",
        ] {
            if parts.iter().any(|p| p == c) {
                return (c.to_string(), true);
            }
        }
        ("customXml/_rels/item1.xml.rels".to_string(), false)
    }
}

pub(crate) fn insert_raw_before_end(
    xml: &[u8],
    closing: &[u8],
    elements: &str,
) -> OL2WMResult<Vec<u8>> {
    let pos = xml
        .windows(closing.len())
        .position(|w| w == closing)
        .ok_or(Error::Xml { stage: "parent-end-not-found" })?;
    let mut out = Vec::with_capacity(xml.len() + elements.len());
    out.extend_from_slice(&xml[..pos]);
    out.extend_from_slice(elements.as_bytes());
    out.extend_from_slice(&xml[pos..]);
    Ok(out)
}

fn check_well_formed(xml: &[u8]) -> OL2WMResult<()> {
    // lightweight tag-balance scan (no external dep): every <x> closed.
    let mut depth = 0i32;
    let mut i = 0;
    while i < xml.len() {
        if xml[i] == b'<' && i + 1 < xml.len() {
            match xml[i + 1] {
                b'?' => {
                    if let Some(e) = find_sub(&xml[i..], b"?>") {
                        i += e + 2;
                        continue;
                    }
                    return Err(Error::Xml { stage: "well-formedness-check" });
                }
                b'!' => {
                    if let Some(e) = find_sub(&xml[i..], b">") {
                        i += e + 1;
                        continue;
                    }
                    return Err(Error::Xml { stage: "well-formedness-check" });
                }
                b'/' => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(Error::Xml { stage: "well-formedness-check" });
                    }
                }
                _ => {
                    // self-closing check at tag end
                    if let Some(e) = find_sub(&xml[i..], b">") {
                        if xml[i + e - 1] != b'/' {
                            depth += 1;
                        }
                        i += e + 1;
                        continue;
                    }
                    return Err(Error::Xml { stage: "well-formedness-check" });
                }
            }
        }
        i += 1;
    }
    if depth != 0 {
        return Err(Error::Xml { stage: "well-formedness-check" });
    }
    Ok(())
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

// ---------- attribute channel (sage:wm in XML roots) ----------

pub const SAGE_NS: &str = "http://sagex/wm";

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(b: &[u8]) -> String {
    let mut out = String::new();
    for c in b.chunks(3) {
        let mut n: u32 = 0;
        for &x in c {
            n = (n << 8) | x as u32;
        }
        n <<= (3 - c.len()) * 8;
        let chars = match c.len() {
            3 => vec![(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63],
            2 => vec![(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63],
            _ => vec![(n >> 18) & 63, (n >> 12) & 63],
        };
        for v in chars {
            out.push(B64[v as usize] as char);
        }
        for _ in 0..(3 - c.len()) {
            out.push('=');
        }
    }
    out
}

fn b64_decode(s: &str) -> OL2WMResult<Vec<u8>> {
    let mut vals: Vec<u32> = Vec::new();
    let mut pad = 0;
    for ch in s.chars() {
        if ch == '=' {
            pad += 1;
            vals.push(0);
            continue;
        }
        let v = B64.iter().position(|&c| c as char == ch).ok_or(Error::BadCover {
            detail: "bad base64 char in sage:wm",
        })? as u32;
        vals.push(v);
    }
    if vals.len() % 4 != 0 {
        return Err(Error::BadCover { detail: "bad base64 length in sage:wm" });
    }
    let mut out = Vec::new();
    for g in vals.chunks(4) {
        let n = (g[0] << 18) | (g[1] << 12) | (g[2] << 6) | g[3];
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
        out.push(n as u8);
    }
    for _ in 0..pad {
        out.pop();
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct AttrChunk {
    pub seq: usize,
    pub rep: usize,
    pub total: usize,
    pub b64: String,
}

#[derive(Debug, Clone)]
pub struct AttrWatermark {
    pub assignments: Vec<(String, Vec<AttrChunk>)>,
    pub payload_len: usize,
}

impl AttrWatermark {
    pub fn new(
        payload: Vec<u8>,
        parts: &[String],
        kind: ContainerKind,
        rep: usize,
    ) -> OL2WMResult<Self> {
        if !ALLOWED.contains(&payload.len()) {
            return Err(Error::BadSize { got: payload.len() });
        }
        let eligible = Self::eligible_parts(kind, parts);
        if eligible.is_empty() {
            return Err(Error::MissingPart { name: "*.xml".into() });
        }
        let rep = rep.clamp(1, 5);
        let b64 = b64_encode(&payload);
        let per = eligible.len().max(1);
        let size = ((b64.len() + per - 1) / per).clamp(16, 512);
        let mut chunks: Vec<String> = Vec::new();
        let mut rest = b64.as_str();
        while !rest.is_empty() {
            let k = size.min(rest.len());
            chunks.push(rest[..k].to_string());
            rest = &rest[k..];
        }
        let total = chunks.len();
        let mut per_part: Vec<Vec<AttrChunk>> = vec![Vec::new(); eligible.len()];
        for (i, ch) in chunks.iter().enumerate() {
            for r in 0..rep {
                let idx = (i * rep + r * 7 + i) % eligible.len();
                if !per_part[idx].iter().any(|c: &AttrChunk| c.seq == i) {
                    per_part[idx].push(AttrChunk { seq: i, rep: r, total, b64: ch.clone() });
                }
            }
        }
        let assignments = eligible.into_iter().zip(per_part).filter(|(_, c)| !c.is_empty()).collect();
        Ok(Self { assignments, payload_len: payload.len() })
    }

    fn eligible_parts(kind: ContainerKind, parts: &[String]) -> Vec<String> {
        parts
            .iter()
            .filter(|p| {
                let p = p.as_str();
                if p == "[Content_Types].xml" || p == "mimetype" {
                    return false;
                }
                if matches!(kind, ContainerKind::Odf) && p == "META-INF/manifest.xml" {
                    return false;
                }
                p.ends_with(".xml") || p.ends_with(".rels")
            })
            .cloned()
            .collect()
    }

    fn splice_root(xml: &[u8], attrs: &str) -> OL2WMResult<Vec<u8>> {
        let mut i = 0;
        while i < xml.len() {
            if xml[i] != b'<' {
                i += 1;
                continue;
            }
            if i + 1 >= xml.len() {
                break;
            }
            match xml[i + 1] {
                b'?' => {
                    if let Some(e) = find_sub(&xml[i..], b"?>") {
                        i += e + 2;
                        continue;
                    }
                    return Err(Error::Xml { stage: "attr-decl" });
                }
                b'!' => {
                    if let Some(e) = find_sub(&xml[i..], b">") {
                        i += e + 1;
                        continue;
                    }
                    return Err(Error::Xml { stage: "attr-doctype" });
                }
                b'/' => return Err(Error::Xml { stage: "attr-no-root" }),
                _ => break,
            }
        }
        if i >= xml.len() || xml[i] != b'<' {
            return Err(Error::Xml { stage: "attr-no-root" });
        }
        let mut j = i + 1;
        let mut q = 0u8;
        while j < xml.len() {
            let c = xml[j];
            if q != 0 {
                if c == q {
                    q = 0;
                }
            } else if c == b'"' || c == b'\'' {
                q = c;
            } else if c == b'>' {
                break;
            }
            j += 1;
        }
        if j >= xml.len() {
            return Err(Error::Xml { stage: "attr-unclosed-root" });
        }
        let mut out = Vec::with_capacity(xml.len() + attrs.len() + 64);
        out.extend_from_slice(&xml[..j]);
        if xml[j - 1] == b'/' {
            out.pop();
            out.extend_from_slice(b" ");
            out.extend_from_slice(b"xmlns:sage=\"http://sagex/wm\"");
            out.extend_from_slice(attrs.as_bytes());
            out.extend_from_slice(b"/>");
        } else {
            out.extend_from_slice(b" ");
            out.extend_from_slice(b"xmlns:sage=\"http://sagex/wm\"");
            out.extend_from_slice(attrs.as_bytes());
            out.extend_from_slice(b">");
        }
        out.extend_from_slice(&xml[j + 1..]);
        Ok(out)
    }

    fn attrs_for(chunks: &[AttrChunk]) -> String {
        let mut s = String::new();
        for (k, c) in chunks.iter().enumerate() {
            s.push_str(&format!(
                " sage:wm{k}=\"{}\" sage:wm-s{k}=\"{}/{}\" sage:wm-r{k}=\"{}\"",
                c.b64, c.seq, c.total, c.rep
            ));
        }
        s
    }

    pub fn apply_to(&self, part: &str, xml: &[u8]) -> OL2WMResult<Vec<u8>> {
        let chunks = self
            .assignments
            .iter()
            .find(|(p, _)| p == part)
            .map(|(_, c)| c)
            .ok_or(Error::MissingPart { name: part.into() })?;
        let out = Self::splice_root(xml, &Self::attrs_for(chunks))?;
        check_well_formed(&out)?;
        Ok(out)
    }

    pub fn decode(scanned: &[(String, Vec<u8>)]) -> OL2WMResult<Vec<u8>> {
        use std::collections::HashMap;
        let mut map: HashMap<usize, (usize, String)> = HashMap::new();
        for (_, xml) in scanned {
            for (_, seq, total, _, b64) in Self::scan_attrs(xml) {
                map.entry(seq).or_insert((total, b64));
            }
        }
        if map.is_empty() {
            return Err(Error::NoWatermark);
        }
        let total = map.values().next().map(|(t, _)| *t).unwrap_or(0);
        let mut seqs: Vec<usize> = map.keys().cloned().collect();
        seqs.sort_unstable();
        if total > 0 && seqs.len() == total && seqs.iter().enumerate().all(|(i, s)| *s == i) {
            let cat: String = seqs.iter().map(|s| map[s].1.as_str()).collect();
            return b64_decode(&cat);
        }
        Err(Error::BadCover { detail: "sage:wm chunks incomplete in all reps" })
    }

    fn scan_attrs(xml: &[u8]) -> Vec<(usize, usize, usize, usize, String)> {
        use std::collections::HashMap;
        let mut out = Vec::new();
        let mut vals: HashMap<String, String> = HashMap::new();
        let mut i = 0;
        while i < xml.len() {
            if xml[i..].starts_with(b"sage:wm") {
                let mut j = i + 7;
                let mut key = String::from("wm");
                if j < xml.len() && xml[j] == b'-' && j + 1 < xml.len()
                    && (xml[j + 1] == b's' || xml[j + 1] == b'r')
                {
                    let kind = xml[j + 1] as char;
                    let mut k = j + 2;
                    let mut num = String::new();
                    while k < xml.len() && xml[k].is_ascii_digit() {
                        num.push(xml[k] as char);
                        k += 1;
                    }
                    key = format!("{kind}-{num}");
                    j = k;
                } else if j < xml.len() && xml[j] == b'-' {
                    let mut k = j + 1;
                    let mut num = String::new();
                    while k < xml.len() && xml[k].is_ascii_digit() {
                        num.push(xml[k] as char);
                        k += 1;
                    }
                    if !num.is_empty() {
                        key = format!("w-{num}");
                        j = k;
                    }
                } else if j < xml.len() && xml[j].is_ascii_digit() {
                    let mut num = String::new();
                    while j < xml.len() && xml[j].is_ascii_digit() {
                        num.push(xml[j] as char);
                        j += 1;
                    }
                    key = format!("w-{num}");
                }
                while j < xml.len() && xml[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < xml.len() && xml[j] == b'=' {
                    j += 1;
                    while j < xml.len() && xml[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < xml.len() && (xml[j] == b'"' || xml[j] == b'\'') {
                        let q = xml[j];
                        j += 1;
                        let start = j;
                        while j < xml.len() && xml[j] != q {
                            j += 1;
                        }
                        vals.insert(key, String::from_utf8_lossy(&xml[start..j]).into_owned());
                        i = j;
                        continue;
                    }
                }
            }
            i += 1;
        }
        let mut per: HashMap<String, (usize, usize, usize, String)> = HashMap::new();
        for (k, v) in &vals {
            if let Some(n) = k.strip_prefix("w-") {
                let e = per.entry(n.to_string()).or_insert((0, 0, 0, String::new()));
                e.3 = v.clone();
            } else if let Some(n) = k.strip_prefix("s-") {
                let mut it = v.split('/');
                let e = per.entry(n.to_string()).or_insert((0, 0, 0, String::new()));
                e.0 = it.next().unwrap_or("0").parse().unwrap_or(0);
                e.1 = it.next().unwrap_or("0").parse().unwrap_or(0);
            } else if let Some(n) = k.strip_prefix("r-") {
                let e = per.entry(n.to_string()).or_insert((0, 0, 0, String::new()));
                e.2 = v.parse().unwrap_or(0);
            }
        }
        for (k, (seq, total, rep, b64)) in per {
            if !b64.is_empty() && total > 0 {
                out.push((k.parse().unwrap_or(0), seq, total, rep, b64));
            }
        }
        out
    }
}

// ---------- image channel (pixel-literal PNG + tiny drawing) ----------

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    for c in raw.chunks(65535) {
        let last = c.as_ptr() as usize + c.len() == raw.as_ptr() as usize + raw.len();
        out.push(if last { 0x01 } else { 0x00 });
        out.extend_from_slice(&(c.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!c.len() as u16).to_le_bytes());
        out.extend_from_slice(c);
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn png_image(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(rgba.len() as u32, w * h * 4);
    let mut raw = Vec::new();
    for row in rgba.chunks_exact(w as usize * 4) {
        raw.push(0); // filter None
        raw.extend_from_slice(row);
    }
    let mut out = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    out.extend(png_chunk(b"IHDR", &ihdr));
    out.extend(png_chunk(b"IDAT", &zlib_stored(&raw)));
    out.extend(png_chunk(b"IEND", &[]));
    out
}

fn png_pixels(png: &[u8]) -> OL2WMResult<(u32, u32, Vec<u8>)> {
    if png.len() < 8 || png[..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err(Error::BadCover { detail: "not a PNG" });
    }
    let mut p = 8;
    let (mut w, mut h) = (0u32, 0u32);
    let mut depth = 0u8;
    let mut ctype = 0u8;
    let mut idat = Vec::new();
    while p + 8 <= png.len() {
        let len =
            u32::from_be_bytes(png[p..p + 4].try_into().map_err(|_| Error::BadCover {
                detail: "truncated PNG chunk",
            })?) as usize;
        if p + 12 + len > png.len() {
            return Err(Error::BadCover { detail: "truncated PNG chunk" });
        }
        match &png[p + 4..p + 8] {
            b"IHDR" => {
                if len < 13 {
                    return Err(Error::BadCover { detail: "bad IHDR" });
                }
                w = u32::from_be_bytes(png[p + 8..p + 12].try_into().unwrap());
                h = u32::from_be_bytes(png[p + 12..p + 16].try_into().unwrap());
                depth = png[p + 16];
                ctype = png[p + 17];
            }
            b"IDAT" => idat.extend_from_slice(&png[p + 8..p + 8 + len]),
            _ => {}
        }
        p += 12 + len;
    }
    if w == 0 || h == 0 || w * h > 1 << 20 {
        return Err(Error::BadCover { detail: "bad PNG dimensions" });
    }
    // zlib stream: 2-byte header + deflate + adler32
    if idat.len() < 6 {
        return Err(Error::BadCover { detail: "bad IDAT" });
    }
    let body = &idat[2..idat.len() - 4];
    let raw = deflate_decode(body).ok_or(Error::BadCover { detail: "IDAT inflate failed" })?;
    // unfilter; support RGBA8 (6) and RGB8 (2), G8 (0)
    let bpp: usize = match (ctype, depth) {
        (6, 8) => 4,
        (2, 8) => 3,
        (0, 8) => 1,
        _ => return Err(Error::BadCover { detail: "unsupported PNG color type" }),
    };
    let stride = w as usize * bpp;
    let mut px = Vec::with_capacity(h as usize * stride);
    let mut prev = vec![0u8; stride];
    let mut q = 0;
    for _ in 0..h {
        if q + 1 + stride > raw.len() {
            return Err(Error::BadCover { detail: "short scanlines" });
        }
        let f = raw[q];
        q += 1;
        let mut row = vec![0u8; stride];
        for i in 0..stride {
            let a = if i >= bpp { row[i - bpp] } else { 0 };
            let b = prev[i];
            let c = if i >= bpp { prev[i - bpp] } else { 0 };
            row[i] = raw[q + i].wrapping_add(match f {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => paeth(a, b, c),
                _ => return Err(Error::BadCover { detail: "bad filter" }),
            });
        }
        q += stride;
        prev = row.clone();
        px.extend_from_slice(&row);
    }
    // normalize to RGBA bytes for payload read: take raw channel bytes
    Ok((w, h, px))
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a, b, c) = (a as i16, b as i16, c as i16);
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

fn deflate_decode(data: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::DeflateDecoder;
    use std::io::Read;
    let mut d = DeflateDecoder::new(data);
    let mut out = Vec::new();
    d.read_to_end(&mut out).ok()?;
    Some(out)
}

#[derive(Debug, Clone)]
pub struct ImageWatermark {
    pub path: String,
    pub png_bytes: Vec<u8>,
    pub payload_len: usize,
    pub width: u32,
    pub height: u32,
}

impl ImageWatermark {
    fn dims_for(len: usize) -> (u32, u32) {
        // RGBA: 4 bytes/px; header 4 + payload, padded to 4
        let total = (4 + len + 3) / 4 * 4;
        let px = (total / 4) as u32;
        let w = (px as f64).sqrt().ceil() as u32;
        let h = (px + w - 1) / w;
        (w.max(1), h.max(1))
    }

    pub fn new(payload: Vec<u8>) -> OL2WMResult<Self> {
        if !ALLOWED.contains(&payload.len()) {
            return Err(Error::BadSize { got: payload.len() });
        }
        let h = hash16(&payload);
        let (w, hh) = Self::dims_for(payload.len());
        let mut raw = Vec::new();
        raw.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        raw.extend_from_slice(&crc16(&payload).to_le_bytes());
        raw.extend_from_slice(&payload);
        while raw.len() < (w * hh * 4) as usize {
            raw.push(0);
        }
        let png_bytes = png_image(w, hh, &raw);
        Ok(Self {
            path: format!("media/wmimg{h:04}.png"),
            png_bytes,
            payload_len: payload.len(),
            width: w,
            height: hh,
        })
    }

    pub fn media_path_for(&self, kind: ContainerKind) -> String {
        match kind {
            ContainerKind::OpcDocx => format!("word/{}", self.path),
            ContainerKind::OpcXlsx => format!("xl/{}", self.path),
            ContainerKind::OpcPptx => format!("ppt/{}", self.path),
            _ => format!("media/{}", self.path.split('/').last().unwrap_or("wm.png")),
        }
    }

    pub fn payload_from_png(png: &[u8]) -> OL2WMResult<Vec<u8>> {
        let (_, _, px) = png_pixels(png)?;
        // RGBA stride: payload is channel-concatenated? we stored RGBA quads
        // in order, so the byte stream is already contiguous
        if px.len() < 4 {
            return Err(Error::NoWatermark);
        }
        let len = u16::from_le_bytes([px[0], px[1]]) as usize;
        let crc = u16::from_le_bytes([px[2], px[3]]);
        if !ALLOWED.contains(&len) || px.len() < 4 + len {
            return Err(Error::NoWatermark);
        }
        let payload = px[4..4 + len].to_vec();
        if crc16(&payload) != crc {
            return Err(Error::BadCover { detail: "image payload crc mismatch" });
        }
        Ok(payload)
    }

    pub fn to_local(&self, path: &str) -> LocalFileHeader {
        LocalFileHeader {
            version: 10,
            flags: zero_flags(),
            compression_method: CompressionMethod::None,
            mod_time: DosTime { raw: 0 },
            mod_date: DosDate { raw: 0x21 },
            crc32: crc32(&self.png_bytes),
            compressed_size: self.png_bytes.len() as u32,
            uncompressed_size: self.png_bytes.len() as u32,
            file_name: path.as_bytes().to_vec(),
            extra: ExtraFields::default(),
            data: self.png_bytes.clone(),
        }
    }

    pub fn media_rel_id(&self) -> String {
        format!("rWmI{:04}", hash16(&self.png_bytes) & 0xffff)
    }

    pub fn custom_rel_id(&self) -> String {
        format!("rWmJ{:04}", hash16(&self.png_bytes) & 0xffff)
    }
}
