//! `office-watermark`: inject a text watermark into Office documents.
//!
//! Handles `docx / pptx / xlsx` (OOXML) and `ods / odt / odp` (ODF) — all of
//! which are zip containers of XML parts.
//!
//! For **every** embedded `*.xml` / `*.rels` entry, exactly one namespaced
//! attribute is injected on the root element:
//!
//! ```xml
//! <w:document ... xmlns:sagex="https://sagex/enc" sagex:wm="YOUR-TEXT">
//! ```
//!
//! Foreign namespaced attributes are ignored for rendering but preserved by
//! Word / Excel / PowerPoint / LibreOffice, so files stay openable.
//!
//! All zip entries are **rewritten with recalculated CRC32** (never raw-copied),
//! original order / compression / mtime / permissions preserved, `mimetype`
//! kept first + `Stored` per the ODF spec. Large files stream with constant
//! memory (`BufReader 64KB`, event-by-event XML).
//!
//! Empty (0-byte) XML parts, e.g. LibreOffice's
//! `Configurations2/accelerator/current.xml`, have no root element to carry
//! an attribute and are passed through unchanged; every non-empty XML part
//! is watermarked.

mod error;
mod extract;
mod format;
mod options;
mod xml_inject;
mod zipio;

pub use error::{Error, Result};
pub use extract::{
    extract_watermarks_bytes, extract_watermarks_file, extract_watermarks_stream, verify_all_equal,
    WatermarkHit,
};
pub use format::OfficeFormat;
pub use options::{WatermarkOptions, MAX_WATERMARK_LEN, WM_ATTR, WM_NS, WM_PREFIX};
pub use xml_inject::inject_slice;
pub use zipio::is_xml_entry;

use std::fs::File;
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;

/// Watermark raw bytes (`&[u8]` zip container) → fresh watermarked bytes.
///
/// Convenience wrapper over [`watermark_stream`] for small files/tests.
pub fn watermark_bytes(input: &[u8], opts: &WatermarkOptions) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Err(Error::EmptyInput);
    }
    zipio::check_magic(&input[..input.len().min(8)])?;
    let reader = Cursor::new(input);
    let mut out = Cursor::new(Vec::with_capacity(input.len().saturating_add(4096)));
    watermark_stream(reader, &mut out, opts)?;
    Ok(out.into_inner())
}

/// Streaming watermark between any seekable reader/writer.
///
/// Returns the detected [`OfficeFormat`]. Uses constant memory; suitable for
/// hundreds-of-MB documents.
pub fn watermark_stream<R: Read + Seek, W: Write + Seek>(
    reader: R,
    writer: W,
    opts: &WatermarkOptions,
) -> Result<OfficeFormat> {
    zipio::watermark_stream(reader, writer, opts)
}

/// Watermark a file on disk → new file on disk (streaming, constant memory).
///
/// Creates parent directories for `output` if needed.
pub fn watermark_file(
    input: &Path,
    output: &Path,
    opts: &WatermarkOptions,
) -> Result<OfficeFormat> {
    let mut header = [0u8; 8];
    {
        let mut f = File::open(input)?;
        let _ = f.read(&mut header);
    }
    zipio::check_magic(&header)?;

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let reader = File::open(input)?;
    let writer = File::create(output)?;
    zipio::watermark_stream(reader, writer, opts)
}
