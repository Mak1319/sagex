//! Invisible forensic watermark for ZIP-based files (`zip`, `docx`, `xlsx`, `pptx`).
//!
//! Strategy: duplicate the same `MAGIC + postcard(Watermark)` block at two
//! places — immediately before the second local file header and immediately
//! before the central directory (if there is no second local header, both
//! copies sit back-to-back before the central directory) — then shift every
//! `CDFH.localHeaderOffset` at or after the first insert by one block and
//! `EOCD.CDOffset` (plus ZIP64 `EOCD64` / locator) by two blocks. Local file
//! headers themselves carry no absolute offsets and are copied verbatim, and
//! the file keeps starting with `PK\x03\x04`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Magic prefix. Not counted in payload size.
pub const MAGIC: [u8; 4] = [0x5A, 0x6E, 0x10, 0xFF];

/// Wire sizes: `MAGIC.len() + postcard(payload)`.
/// Payload is 1-byte enum tag + 16 or 256 raw bytes (no length prefix).
pub const UUID_WIRE_LEN: usize = 4 + 1 + 16;
pub const RAW_WIRE_LEN: usize = 4 + 1 + 256;

/// Forensic watermark payload, serialized with `postcard`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watermark {
    Uuid([u8; 16]),
    Raw([u8; 256]),
}

/// `serde` only implements arrays up to 32 elements, so the 256-byte array
/// is coded by hand as a 256-tuple (same layout `postcard` would use).
struct AsTuple256<'a>(&'a [u8; 256]);

impl Serialize for AsTuple256<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut t = s.serialize_tuple(256)?;
        for b in self.0.iter() {
            t.serialize_element(b)?;
        }
        t.end()
    }
}

struct DeTuple256;

impl<'de> serde::de::Visitor<'de> for DeTuple256 {
    type Value = [u8; 256];

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("256 bytes")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<[u8; 256], A::Error> {
        let mut arr = [0u8; 256];
        for (i, slot) in arr.iter_mut().enumerate() {
            *slot = seq
                .next_element()?
                .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
        }
        Ok(arr)
    }
}

fn de_tuple256<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 256], D::Error> {
    d.deserialize_tuple(256, DeTuple256)
}

impl Serialize for Watermark {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Watermark::Uuid(id) => s.serialize_newtype_variant("Watermark", 0, "Uuid", id),
            Watermark::Raw(arr) => {
                s.serialize_newtype_variant("Watermark", 1, "Raw", &AsTuple256(arr))
            }
        }
    }
}

struct WatermarkVisitor;

impl<'de> serde::de::Visitor<'de> for WatermarkVisitor {
    type Value = Watermark;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("enum Watermark")
    }

    fn visit_enum<A: serde::de::EnumAccess<'de>>(
        self,
        data: A,
    ) -> Result<Watermark, A::Error> {
        use serde::de::VariantAccess;
        let (tag, variant) = data.variant::<u32>()?;
        match tag {
            0 => {
                let id: [u8; 16] = variant.newtype_variant()?;
                Ok(Watermark::Uuid(id))
            }
            1 => {
                let arr = variant.newtype_variant_seed(DeSeed256)?;
                Ok(Watermark::Raw(arr))
            }
            _ => Err(serde::de::Error::unknown_variant(
                &tag.to_string(),
                &["Uuid", "Raw"],
            )),
        }
    }
}

struct DeSeed256;

impl<'de> serde::de::DeserializeSeed<'de> for DeSeed256 {
    type Value = [u8; 256];

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<[u8; 256], D::Error> {
        de_tuple256(d)
    }
}

impl<'de> Deserialize<'de> for Watermark {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Watermark, D::Error> {
        d.deserialize_enum("Watermark", &["Uuid", "Raw"], WatermarkVisitor)
    }
}

impl Watermark {
    fn payload_len(&self) -> usize {
        match self {
            Watermark::Uuid(_) => 1 + 16,
            Watermark::Raw(_) => 1 + 256,
        }
    }

    pub fn wire_len(&self) -> usize {
        MAGIC.len() + self.payload_len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    EmptyInput,
    NoEocd,
    BadCd,
    UnsupportedMultiDisk,
    BadLocator,
    BadEocd64,
    Truncated,
    CdOffsetOverflow,
    LocalOffsetOverflow,
    BadLocalHeader,
    NoMagicAtSecondLfh,
    NoMagicAtCd,
    InvalidPayload,
    WatermarkMismatch,
    SerializeFailed,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Error::EmptyInput => "empty input",
            Error::NoEocd => "end-of-central-directory not found",
            Error::BadCd => "central directory out of bounds or corrupt",
            Error::UnsupportedMultiDisk => "multi-disk archives not supported",
            Error::BadLocator => "zip64 locator invalid",
            Error::BadEocd64 => "zip64 end-of-central-directory invalid",
            Error::Truncated => "truncated archive",
            Error::CdOffsetOverflow => "central directory offset does not fit",
            Error::LocalOffsetOverflow => "local header offset does not fit",
            Error::BadLocalHeader => "second local file header not found",
            Error::NoMagicAtSecondLfh => "watermark magic missing before second local header",
            Error::NoMagicAtCd => "watermark magic missing before central directory",
            Error::InvalidPayload => "watermark payload invalid",
            Error::WatermarkMismatch => "offset-0 and pre-CD watermarks differ",
            Error::SerializeFailed => "watermark serialization failed",
        };
        f.write_str(s)
    }
}

impl std::error::Error for Error {}

const SIG_EOCD32: u32 = 0x0605_4B50;
const SIG_EOCD64: u32 = 0x0606_4B50;
const SIG_LOCATOR: u32 = 0x0706_4B50;
const SIG_CDH: u32 = 0x0201_4B50;
const SIG_LFH: u32 = 0x0403_4B50;

const EOCD32_MIN: usize = 22;
const LOCATOR_LEN: usize = 20;
const CDFH_FIXED: usize = 46;

fn read_u16_le(b: &[u8], off: usize) -> Result<u16, Error> {
    b.get(off..off + 2)
        .ok_or(Error::Truncated)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32_le(b: &[u8], off: usize) -> Result<u32, Error> {
    b.get(off..off + 4)
        .ok_or(Error::Truncated)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u64_le(b: &[u8], off: usize) -> Result<u64, Error> {
    b.get(off..off + 8)
        .ok_or(Error::Truncated)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}

fn write_u32_le(b: &mut [u8], off: usize, v: u32) -> Result<(), Error> {
    b.get_mut(off..off + 4)
        .ok_or(Error::Truncated)
        .map(|s| s.copy_from_slice(&v.to_le_bytes()))
}

fn write_u64_le(b: &mut [u8], off: usize, v: u64) -> Result<(), Error> {
    b.get_mut(off..off + 8)
        .ok_or(Error::Truncated)
        .map(|s| s.copy_from_slice(&v.to_le_bytes()))
}

struct Eocd {
    cd_off: u64,
    cd_size: u64,
    count: u64,
    eocd32_off: usize,
    eocd64_off: Option<usize>,
    locator_off: Option<usize>,
}

fn parse_eocd32_at(input: &[u8], off: usize) -> Result<(u16, u16, u32, u32), Error> {
    if read_u32_le(input, off)? != SIG_EOCD32 {
        return Err(Error::NoEocd);
    }
    let disk_num = read_u16_le(input, off + 4)?;
    let disk_start = read_u16_le(input, off + 6)?;
    let cdr_disk = read_u16_le(input, off + 8)?;
    let cdr_total = read_u16_le(input, off + 10)?;
    let cd_size = read_u32_le(input, off + 12)?;
    let cd_off = read_u32_le(input, off + 16)?;
    let comment_len = read_u16_le(input, off + 20)? as usize;
    if input.len().checked_sub(off).unwrap_or(0) < EOCD32_MIN + comment_len {
        return Err(Error::Truncated);
    }
    if disk_num != 0 || disk_start != 0 || cdr_disk != cdr_total {
        return Err(Error::UnsupportedMultiDisk);
    }
    Ok((cdr_total, cdr_total, cd_size, cd_off))
}

fn cd_magic_ok(input: &[u8], cd_off: u64) -> bool {
    let off = cd_off as usize;
    if cd_off > usize::MAX as u64 {
        return false;
    }
    matches!(input.get(off..off + 4), Some(s) if u32::from_le_bytes([s[0], s[1], s[2], s[3]]) == SIG_CDH)
        || (cd_off == input.len() as u64)
}

fn parse_eocd64_at(input: &[u8], off: usize) -> Result<(u64, u64, u64), Error> {
    if read_u32_le(input, off)? != SIG_EOCD64 {
        return Err(Error::BadEocd64);
    }
    let eocd_size = read_u64_le(input, off + 4)?;
    if eocd_size < 44 {
        return Err(Error::BadEocd64);
    }
    let disk_num = read_u32_le(input, off + 16)?;
    let disk_start = read_u32_le(input, off + 20)?;
    if disk_num != 0 || disk_start != 0 {
        return Err(Error::UnsupportedMultiDisk);
    }
    let count = read_u64_le(input, off + 24)?;
    let total = read_u64_le(input, off + 32)?;
    if count != total {
        return Err(Error::UnsupportedMultiDisk);
    }
    let cd_size = read_u64_le(input, off + 40)?;
    let cd_off = read_u64_le(input, off + 48)?;
    let record_end = (off as u64)
        .checked_add(12 + eocd_size)
        .ok_or(Error::BadEocd64)?;
    if record_end > input.len() as u64 {
        return Err(Error::Truncated);
    }
    Ok((count, cd_size, cd_off))
}

fn find_eocd(input: &[u8]) -> Result<Eocd, Error> {
    if input.len() < EOCD32_MIN {
        return Err(Error::NoEocd);
    }
    let search_from = input.len().saturating_sub(65535 + EOCD32_MIN);
    let mut off = input.len() - 4;
    loop {
        if off >= search_from
            && input.get(off..off + 4) == Some(&[0x50, 0x4B, 0x05, 0x06])
            && EOCD32_MIN + off <= input.len()
        {
            let cd_off32 = read_u32_le(input, off + 16).unwrap_or(0);
            let cdr_total = read_u16_le(input, off + 10).unwrap_or(0xFFFF);
            let is_zip64 = cd_off32 == 0xFFFF_FFFF || cdr_total == 0xFFFF;
            if is_zip64 {
                if off >= LOCATOR_LEN {
                    let loc = off - LOCATOR_LEN;
                    if read_u32_le(input, loc).unwrap_or(0) == SIG_LOCATOR {
                        let eocd64_off = read_u64_le(input, loc + 8).unwrap_or(0) as usize;
                        if read_u32_le(input, eocd64_off).unwrap_or(0) == SIG_EOCD64 {
                            if let Ok((count, cd_size, cd_off)) =
                                parse_eocd64_at(input, eocd64_off)
                            {
                                let locator_eocd = read_u64_le(input, loc + 8).unwrap_or(0);
                                if locator_eocd as usize == eocd64_off
                                    && (cd_magic_ok(input, cd_off) || count == 0)
                                    && (eocd64_off as u64) < loc as u64
                                {
                                    return Ok(Eocd {
                                        cd_off,
                                        cd_size,
                                        count,
                                        eocd32_off: off,
                                        eocd64_off: Some(eocd64_off),
                                        locator_off: Some(loc),
                                    });
                                }
                            }
                        }
                    }
                }
            } else if let Ok((_, count, cd_size, cd_off)) = parse_eocd32_at(input, off) {
                if cd_magic_ok(input, cd_off as u64) || count == 0 {
                    let cd_off = cd_off as u64;
                    if cd_off + cd_size as u64 <= off as u64 {
                        return Ok(Eocd {
                            cd_off,
                            cd_size: cd_size as u64,
                            count: count as u64,
                            eocd32_off: off,
                            eocd64_off: None,
                            locator_off: None,
                        });
                    }
                }
            }
        }
        if off == search_from || off == 0 {
            break;
        }
        off -= 1;
    }
    Err(Error::NoEocd)
}

/// Byte offset where the central directory starts.
pub fn cd_offset(input: &[u8]) -> Result<u64, Error> {
    if input.is_empty() {
        return Err(Error::EmptyInput);
    }
    Ok(find_eocd(input)?.cd_off)
}

/// Human-facing summary of an archive (powers the `info` command).
#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    pub count: u64,
    pub cd_off: u64,
    pub cd_size: u64,
    pub is_zip64: bool,
    /// Where copy #1 would be inserted (second-LFH offset, or `cd_off`
    /// when there is no second entry).
    pub insert_point: u64,
    /// File name of the second entry, if any.
    pub second_name: Option<String>,
}

pub fn inspect(input: &[u8]) -> Result<ArchiveInfo, Error> {
    if input.is_empty() {
        return Err(Error::EmptyInput);
    }
    let eocd = find_eocd(input)?;
    let insert_point = second_lfh_offset(input, &eocd)?;
    let mut second_name = None;
    if eocd.count >= 2 {
        let old_cd_usize: usize = eocd.cd_off.try_into().map_err(|_| Error::BadCd)?;
        let first_len = cdfh_entry_len(input, old_cd_usize)?;
        let second_off = old_cd_usize.checked_add(first_len).ok_or(Error::BadCd)?;
        let name_len = read_u16_le(input, second_off + 28)? as usize;
        let name_off = second_off + CDFH_FIXED;
        let name_end = name_off.checked_add(name_len).ok_or(Error::BadCd)?;
        let name_bytes = input.get(name_off..name_end).ok_or(Error::BadCd)?;
        second_name = Some(String::from_utf8_lossy(name_bytes).into_owned());
    }
    Ok(ArchiveInfo {
        count: eocd.count,
        cd_off: eocd.cd_off,
        cd_size: eocd.cd_size,
        is_zip64: eocd.eocd64_off.is_some(),
        insert_point,
        second_name,
    })
}

/// Full length of one CDFH entry, including name/extra/comment.
fn cdfh_entry_len(input: &[u8], entry_off: usize) -> Result<usize, Error> {
    if read_u32_le(input, entry_off)? != SIG_CDH {
        return Err(Error::BadCd);
    }
    let name_len = read_u16_le(input, entry_off + 28)? as usize;
    let extra_len = read_u16_le(input, entry_off + 30)? as usize;
    let comment_len = read_u16_le(input, entry_off + 32)? as usize;
    let len = CDFH_FIXED
        .checked_add(name_len)
        .and_then(|v| v.checked_add(extra_len))
        .and_then(|v| v.checked_add(comment_len))
        .ok_or(Error::BadCd)?;
    if entry_off.checked_add(len).ok_or(Error::BadCd)? > input.len() {
        return Err(Error::BadCd);
    }
    Ok(len)
}

/// True local-header offset of a CDFH entry (resolves ZIP64 extra `0x0001`).
fn cdfh_lho(input: &[u8], entry_off: usize) -> Result<u64, Error> {
    let lho = read_u32_le(input, entry_off + 42)?;
    if lho != 0xFFFF_FFFF {
        return Ok(lho as u64);
    }
    let uncomp = read_u32_le(input, entry_off + 24)?;
    let comp = read_u32_le(input, entry_off + 20)?;
    let name_len = read_u16_le(input, entry_off + 28)? as usize;
    let extra_len = read_u16_le(input, entry_off + 30)? as usize;
    let extra_start = entry_off + CDFH_FIXED + name_len;
    let extra_end = extra_start.checked_add(extra_len).ok_or(Error::BadCd)?;
    if extra_end > input.len() {
        return Err(Error::BadCd);
    }
    let mut cur = extra_start;
    while cur + 4 <= extra_end {
        let tag = read_u16_le(input, cur)?;
        let size = read_u16_le(input, cur + 2)? as usize;
        let data_start = cur + 4;
        let data_end = data_start.checked_add(size).ok_or(Error::BadCd)?;
        if data_end > extra_end {
            return Err(Error::BadCd);
        }
        if tag == 0x0001 {
            let mut pos = 0usize;
            if uncomp == 0xFFFF_FFFF {
                pos += 8;
            }
            if comp == 0xFFFF_FFFF {
                pos += 8;
            }
            if pos + 8 > size {
                return Err(Error::BadCd);
            }
            return read_u64_le(input, data_start + pos);
        }
        cur = data_end;
    }
    Err(Error::BadCd)
}

/// Insert point of copy #1: offset of the second LFH, or `old_cd` when the
/// archive has fewer than two entries (both copies go back-to-back pre-CD).
fn second_lfh_offset(input: &[u8], eocd: &Eocd) -> Result<u64, Error> {
    let old_cd_usize: usize = eocd.cd_off.try_into().map_err(|_| Error::BadCd)?;
    if eocd.count < 2 {
        return Ok(eocd.cd_off);
    }
    let cd_size_usize: usize = eocd.cd_size.try_into().map_err(|_| Error::BadCd)?;
    let cd_end = old_cd_usize.checked_add(cd_size_usize).ok_or(Error::BadCd)?;
    let first_len = cdfh_entry_len(input, old_cd_usize)?;
    let second_off = old_cd_usize.checked_add(first_len).ok_or(Error::BadCd)?;
    if second_off.checked_add(CDFH_FIXED).ok_or(Error::BadCd)? > cd_end {
        return Err(Error::BadCd);
    }
    let lho = cdfh_lho(input, second_off)?;
    if lho >= eocd.cd_off {
        return Err(Error::BadLocalHeader);
    }
    let lho_usize: usize = lho.try_into().map_err(|_| Error::BadLocalHeader)?;
    if read_u32_le(input, lho_usize).map_err(|_| Error::BadLocalHeader)? != SIG_LFH {
        return Err(Error::BadLocalHeader);
    }
    Ok(lho)
}

fn encode_wire(wm: &Watermark) -> Result<Vec<u8>, Error> {
    let payload = postcard::to_allocvec(wm).map_err(|_| Error::SerializeFailed)?;
    let mut wire = Vec::with_capacity(MAGIC.len() + payload.len());
    wire.extend_from_slice(&MAGIC);
    wire.extend_from_slice(&payload);
    Ok(wire)
}

fn decode_payload(bytes: &[u8]) -> Result<Watermark, Error> {
    // Exact slice: `from_bytes` ignores trailing input, so length is enforced
    // by the caller slicing precisely one payload.
    postcard::from_bytes::<Watermark>(bytes).map_err(|_| Error::InvalidPayload)
}

/// Embed `wm` before the second LFH and immediately before the central
/// directory (both copies back-to-back pre-CD when there is no second LFH).
pub fn embed(input: &[u8], wm: &Watermark) -> Result<Vec<u8>, Error> {
    if input.is_empty() {
        return Err(Error::EmptyInput);
    }
    let wire = encode_wire(wm)?;
    let n = wire.len() as u64;
    let eocd = find_eocd(input)?;
    let old_cd = eocd.cd_off;
    let old_cd_usize: usize = old_cd.try_into().map_err(|_| Error::BadCd)?;
    if old_cd_usize > input.len() {
        return Err(Error::BadCd);
    }
    let cd_size_usize: usize = eocd.cd_size.try_into().map_err(|_| Error::BadCd)?;
    if old_cd_usize
        .checked_add(cd_size_usize)
        .ok_or(Error::BadCd)?
        > input.len()
    {
        return Err(Error::BadCd);
    }
    let s = second_lfh_offset(input, &eocd)?;
    let s_usize: usize = s.try_into().map_err(|_| Error::BadCd)?;
    if s_usize > old_cd_usize {
        return Err(Error::BadLocalHeader);
    }

    let shift_cd = n.checked_mul(2).ok_or(Error::CdOffsetOverflow)?;
    let new_cd = old_cd.checked_add(shift_cd).ok_or(Error::CdOffsetOverflow)?;

    let mut out = Vec::with_capacity(input.len() + shift_cd as usize);
    out.extend_from_slice(&input[..s_usize]);
    out.extend_from_slice(&wire);
    out.extend_from_slice(&input[s_usize..old_cd_usize]);
    out.extend_from_slice(&wire);
    out.extend_from_slice(&input[old_cd_usize..]);

    patch_central_directory(&mut out, input, &eocd, n, s, shift_cd)?;

    match (eocd.eocd64_off, eocd.locator_off) {
        (Some(e64), Some(loc)) => {
            let e64_out = e64 + shift_cd as usize;
            let loc_out = loc + shift_cd as usize;
            write_u64_le(&mut out, e64_out + 48, new_cd)?;
            let new_e64 = (e64 as u64).checked_add(shift_cd).ok_or(Error::CdOffsetOverflow)?;
            write_u64_le(&mut out, loc_out + 8, new_e64)?;
        }
        _ => {
            let e32_out = eocd.eocd32_off + shift_cd as usize;
            let new_cd32: u32 = new_cd.try_into().map_err(|_| Error::CdOffsetOverflow)?;
            write_u32_le(&mut out, e32_out + 16, new_cd32)?;
        }
    }
    Ok(out)
}

fn patch_central_directory(
    out: &mut [u8],
    input: &[u8],
    eocd: &Eocd,
    n: u64,
    s: u64,
    shift_cd: u64,
) -> Result<(), Error> {
    let old_cd_usize: usize = eocd.cd_off.try_into().map_err(|_| Error::BadCd)?;
    let cd_size_usize: usize = eocd.cd_size.try_into().map_err(|_| Error::BadCd)?;
    let cd_end = old_cd_usize.checked_add(cd_size_usize).ok_or(Error::BadCd)?;
    let shift_cd_usize: usize = shift_cd.try_into().map_err(|_| Error::CdOffsetOverflow)?;
    let mut entry_off = old_cd_usize;
    for _ in 0..eocd.count {
        if entry_off.checked_add(CDFH_FIXED).ok_or(Error::BadCd)? > cd_end {
            return Err(Error::BadCd);
        }
        let entry_len = cdfh_entry_len(input, entry_off)?;
        if entry_off.checked_add(entry_len).ok_or(Error::BadCd)? > cd_end {
            return Err(Error::BadCd);
        }
        let name_len = read_u16_le(input, entry_off + 28)? as usize;
        let extra_len = read_u16_le(input, entry_off + 30)? as usize;
        let lho_val = cdfh_lho(input, entry_off)?;
        // Records strictly before the first insert keep their offset.
        let shift = if lho_val < s { 0 } else { n };

        let out_entry = entry_off + shift_cd_usize;
        let lho = read_u32_le(input, entry_off + 42)?;
        if lho != 0xFFFF_FFFF {
            let new_lho = lho_val.checked_add(shift).ok_or(Error::LocalOffsetOverflow)?;
            let new_lho32: u32 = new_lho.try_into().map_err(|_| Error::LocalOffsetOverflow)?;
            write_u32_le(out, out_entry + 42, new_lho32)?;
        } else {
            patch_zip64_lho(out, input, entry_off, out_entry, extra_len, name_len, shift)?;
        }
        entry_off += entry_len;
    }
    Ok(())
}

fn patch_zip64_lho(
    out: &mut [u8],
    input: &[u8],
    entry_off: usize,
    out_entry: usize,
    extra_len: usize,
    name_len: usize,
    shift_lfh: u64,
) -> Result<(), Error> {
    let uncomp = read_u32_le(input, entry_off + 24)?;
    let comp = read_u32_le(input, entry_off + 20)?;
    let extra_start = entry_off + CDFH_FIXED + name_len;
    let extra_end = extra_start.checked_add(extra_len).ok_or(Error::BadCd)?;
    if extra_end > input.len() {
        return Err(Error::BadCd);
    }
    let mut cur = extra_start;
    while cur + 4 <= extra_end {
        let tag = read_u16_le(input, cur)?;
        let size = read_u16_le(input, cur + 2)? as usize;
        let data_start = cur + 4;
        let data_end = data_start.checked_add(size).ok_or(Error::BadCd)?;
        if data_end > extra_end {
            return Err(Error::BadCd);
        }
        if tag == 0x0001 {
            let mut pos = 0usize;
            if uncomp == 0xFFFF_FFFF {
                pos += 8;
            }
            if comp == 0xFFFF_FFFF {
                pos += 8;
            }
            if pos + 8 > size {
                return Err(Error::BadCd);
            }
            let old = read_u64_le(input, data_start + pos)?;
            let new = old.checked_add(shift_lfh).ok_or(Error::LocalOffsetOverflow)?;
            let out_data = out_entry + CDFH_FIXED + name_len + (data_start - extra_start) + pos;
            write_u64_le(out, out_data, new)?;
            return Ok(());
        }
        cur = data_end;
    }
    Err(Error::BadCd)
}

fn wire_len_from_tag(tag: u8) -> Result<usize, Error> {
    match tag {
        0 => Ok(UUID_WIRE_LEN),
        1 => Ok(RAW_WIRE_LEN),
        _ => Err(Error::InvalidPayload),
    }
}

/// Watermark plus the exact offsets of both copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub watermark: Watermark,
    pub copy_a: u64,
    pub copy_b: u64,
}

/// Extract the watermark, verifying both copies (pre-second-LFH and pre-CD) match.
pub fn extract(leaked: &[u8]) -> Result<Watermark, Error> {
    Ok(locate(leaked)?.watermark)
}

/// Scan a file and trace out the watermark plus both copy offsets.
pub fn locate(leaked: &[u8]) -> Result<Located, Error> {
    let eocd = find_eocd(leaked)?;
    let new_cd: usize = eocd.cd_off.try_into().map_err(|_| Error::BadCd)?;

    // Copy #2 first: its length is self-describing via the tag byte.
    let mut found: Option<(usize, u64, Watermark)> = None;
    for n in [UUID_WIRE_LEN, RAW_WIRE_LEN] {
        let Some(start) = new_cd.checked_sub(n) else {
            continue;
        };
        let Some(end) = start.checked_add(n) else {
            continue;
        };
        if end > leaked.len() || new_cd > leaked.len() {
            continue;
        }
        if leaked[start..start + MAGIC.len()] != MAGIC {
            continue;
        }
        if wire_len_from_tag(leaked[start + MAGIC.len()]).ok() != Some(n) {
            continue;
        }
        found = Some((
            n,
            start as u64,
            decode_payload(&leaked[start + MAGIC.len()..end])?,
        ));
        break;
    }
    let (n, start2, wm_b) = found.ok_or(Error::NoMagicAtCd)?;

    // Copy #1: before the second LFH (whose stored offset already includes
    // the +N shift), or back-to-back with copy #2 when there is no second
    // entry.
    let start_a: usize = if eocd.count >= 2 {
        let cd_size_usize: usize = eocd.cd_size.try_into().map_err(|_| Error::BadCd)?;
        let cd_end = new_cd.checked_add(cd_size_usize).ok_or(Error::BadCd)?;
        let first_len = cdfh_entry_len(leaked, new_cd).map_err(|_| Error::BadLocalHeader)?;
        let second_off = new_cd.checked_add(first_len).ok_or(Error::BadCd)?;
        if second_off.checked_add(CDFH_FIXED).ok_or(Error::BadCd)? > cd_end {
            return Err(Error::BadCd);
        }
        let lho1 = cdfh_lho(leaked, second_off).map_err(|_| Error::BadLocalHeader)?;
        lho1
            .checked_sub(n as u64)
            .ok_or(Error::BadLocalHeader)?
            .try_into()
            .map_err(|_| Error::BadLocalHeader)?
    } else {
        new_cd.checked_sub(2 * n).ok_or(Error::BadCd)?
    };
    let end_a = start_a.checked_add(n).ok_or(Error::BadCd)?;
    if end_a > leaked.len() {
        return Err(Error::BadCd);
    }
    if leaked[start_a..start_a + MAGIC.len()] != MAGIC {
        return Err(Error::NoMagicAtSecondLfh);
    }
    if wire_len_from_tag(leaked[start_a + MAGIC.len()]).ok() != Some(n) {
        return Err(Error::WatermarkMismatch);
    }
    let wm_a = decode_payload(&leaked[start_a + MAGIC.len()..end_a])?;
    if wm_a != wm_b {
        return Err(Error::WatermarkMismatch);
    }
    Ok(Located {
        watermark: wm_a,
        copy_a: start_a as u64,
        copy_b: start2,
    })
}
