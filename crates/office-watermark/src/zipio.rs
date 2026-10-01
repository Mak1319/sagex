use std::io::{BufReader, Read, Seek, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{Error, Result};
use crate::format::{
    detect_from_names, is_ole_magic, is_zip_magic, refine_odf_from_mimetype, OfficeFormat,
};
use crate::options::WatermarkOptions;
use crate::xml_inject::inject_stream;

/// Returns true for entries we watermark: every `*.xml` and `*.rels`.
pub fn is_xml_entry(name: &str) -> bool {
    if name.ends_with('/') {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".xml") || lower.ends_with(".rels")
}

/// Build fresh entry options: same compression/mode/mtime, fresh CRC on write.
fn build_options(
    compression: CompressionMethod,
    unix_mode: u32,
    mtime: Option<zip::DateTime>,
) -> SimpleFileOptions {
    let mut opts = SimpleFileOptions::default()
        .compression_method(compression)
        .unix_permissions(unix_mode)
        .large_file(true);
    if let Some(t) = mtime {
        opts = opts.last_modified_time(t);
    }
    opts
}
///
/// Reads `reader` entry-by-entry and writes a fresh archive to `writer` with
/// **recalculated CRC32/sizes for every entry** (never copies raw CRCs), so
/// output always passes `unzip -t` and opens in Office/LibreOffice.
///
/// Guarantees:
/// * original entry order preserved (`mimetype` stays first for ODF)
/// * `mimetype` forced to `Stored` + never watermarked (ODF spec requirement)
/// * original `compression()`, `mtime`, `unix_mode` preserved otherwise
/// * every `*.xml` / `*.rels` root gets `sagex:wm`
pub fn watermark_stream<R: Read + Seek, W: Write + Seek>(
    reader: R,
    writer: W,
    opts: &WatermarkOptions,
) -> Result<OfficeFormat> {
    let mut archive = ZipArchive::new(reader).map_err(|e| match e {
        zip::result::ZipError::InvalidArchive(msg) => Error::UnsupportedFormat(msg.to_string()),
        other => Error::Zip(other),
    })?;

    if archive.is_empty() {
        return Err(Error::EmptyInput);
    }

    // Collect names first for format detection (borrow ends before loop).
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_owned()))
        .collect();
    let mut format = detect_from_names(&names);

    // Refine ODF flavour from mimetype payload when present.
    if format == OfficeFormat::Ods && names.iter().any(|n| n == "mimetype") {
        if let Ok(mut f) = archive.by_name("mimetype") {
            let mut payload = String::new();
            if f.read_to_string(&mut payload).is_ok() {
                format = refine_odf_from_mimetype(&payload);
            }
        }
    }

    let mut out = ZipWriter::new(writer);

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_owned();
        let is_dir = file.is_dir();
        let orig_compression = file.compression();
        let mtime = file.last_modified();
        let unix_mode = file.unix_mode().unwrap_or(0o644);

        // Directories: recreate, fresh headers/CRC.
        if is_dir {
            let options = build_options(CompressionMethod::Stored, unix_mode, mtime);
            out.add_directory(name, options)?;
            continue;
        }

        // ODF mimetype: MUST be first + Stored + unwatermarked.
        if name == "mimetype" {
            let options = build_options(CompressionMethod::Stored, unix_mode, mtime);
            out.start_file(name, options)?;
            std::io::copy(&mut file, &mut out)?;
            continue;
        }

        let options = build_options(orig_compression, unix_mode, mtime);

        if is_xml_entry(&name) {
            // Empty XML parts (e.g. LibreOffice's 0-byte
            // `Configurations2/accelerator/current.xml`) have no root
            // element to carry an attribute: pass them through unchanged
            // so output stays openable. Non-empty parts are always injected.
            if file.size() == 0 {
                out.start_file(&name, options)?;
                std::io::copy(&mut file, &mut out)?;
                continue;
            }
            out.start_file(&name, options)?;
            // Stream XML -> XML with root-attribute injection. `&mut file`
            // implements Read; wrap in BufReader for quick-xml. `&mut out`
            // implements Write; ZipWriter recomputes CRC on these bytes.
            {
                let input = BufReader::with_capacity(64 * 1024, &mut file);
                // Writer<&mut ZipWriter<W>> writes straight into the entry.
                inject_stream(input, &mut out, &name, opts)?;
            }
        } else {
            // Binary / other parts: byte-identical copy, fresh CRC.
            out.start_file(&name, options)?;
            std::io::copy(&mut file, &mut out)?;
        }
    }

    out.finish()?;
    Ok(format)
}

/// Peek at the first bytes to reject legacy OLE / non-zip inputs early.
pub fn check_magic(header: &[u8]) -> Result<()> {
    if is_ole_magic(header) {
        return Err(Error::UnsupportedFormat(
            "legacy OLE / encrypted Office file (not a zip container)".into(),
        ));
    }
    if header.len() >= 4 && !is_zip_magic(header) {
        return Err(Error::UnsupportedFormat(
            "not a zip container (missing PK header)".into(),
        ));
    }
    Ok(())
}
