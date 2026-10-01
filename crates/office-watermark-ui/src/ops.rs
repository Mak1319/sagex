//! Blocking watermark operations for the GUI thread-pool.
//!
//! Thin wrappers over `office-watermark` with plain result structs, mirroring
//! the `office-watermark-demo` CLI semantics exactly (same validation, same
//! strict-verify rule, same sample builders for roundtrip self-test).

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use office_watermark::{
    extract_watermarks_bytes, extract_watermarks_file, verify_all_equal, watermark_bytes,
    watermark_file, WatermarkHit, WatermarkOptions,
};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

#[derive(Debug, Clone)]
pub struct EncodeResult {
    pub format: String,
    pub watermarked_parts: usize,
}

#[derive(Debug, Clone)]
pub struct DecodeResult {
    pub hits: Vec<WatermarkHit>,
    /// Single agreed value, if every hit agrees.
    pub common: Option<String>,
    /// Distinct values, when hits disagree.
    pub distinct: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub hits: Vec<WatermarkHit>,
    pub reference: String,
    pub matched: usize,
    pub total_xml: usize,
    pub missing: usize,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct RoundtripResult {
    pub format: String,
    pub text: String,
    pub hits: Vec<WatermarkHit>,
    pub ok: bool,
}

pub fn validate_text(text: &str) -> Result<(), String> {
    WatermarkOptions::new(text)
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

pub fn op_encode(input: &Path, output: &Path, text: &str) -> Result<EncodeResult, String> {
    let opts = WatermarkOptions::new(text).map_err(|e| format!("{e:#}"))?;
    let format = watermark_file(input, output, &opts).map_err(|e| format!("{e:#}"))?;
    let hits = extract_watermarks_file(output).map_err(|e| format!("{e:#}"))?;
    Ok(EncodeResult {
        format: format!("{format:?}"),
        watermarked_parts: hits.len(),
    })
}

pub fn op_decode(file: &Path) -> Result<DecodeResult, String> {
    let hits = extract_watermarks_file(file).map_err(|e| format!("{e:#}"))?;
    if hits.is_empty() {
        return Err("no sagex:wm watermarks found".to_string());
    }
    Ok(summarize_hits(hits))
}

fn summarize_hits(hits: Vec<WatermarkHit>) -> DecodeResult {
    let first = hits.first().map(|h| h.value.clone()).unwrap_or_default();
    if verify_all_equal(&hits, &first) {
        DecodeResult { hits, common: Some(first), distinct: vec![] }
    } else {
        let mut distinct = Vec::new();
        for h in &hits {
            if !distinct.contains(&h.value) {
                distinct.push(h.value.clone());
            }
        }
        DecodeResult { hits, common: None, distinct }
    }
}

pub fn op_verify(file: &Path) -> Result<VerifyResult, String> {
    let hits = extract_watermarks_file(file).map_err(|e| format!("{e:#}"))?;
    let total_xml = count_xml_entries(file).map_err(|e| format!("{e:#}"))?;
    let missing = total_xml.saturating_sub(hits.len());
    let reference = hits.first().map(|h| h.value.clone()).unwrap_or_default();
    let matched = hits.iter().filter(|h| h.value == reference).count();
    let ok = verify_all_equal(&hits, &reference) && missing == 0;
    Ok(VerifyResult { hits, reference, matched, total_xml, missing, ok })
}

/// Count watermarkable entries (mirrors the demo CLI rule).
fn count_xml_entries(path: &Path) -> anyhow::Result<usize> {
    let f = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(f)?;
    let mut n = 0;
    for i in 0..z.len() {
        let e = z.by_index(i)?;
        let name = e.name().to_owned();
        if name == "mimetype" || name.ends_with('/') || e.size() == 0 {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".xml") || lower.ends_with(".rels") {
            n += 1;
        }
    }
    Ok(n)
}

pub fn op_roundtrip(format: &str, text: &str) -> Result<RoundtripResult, String> {
    let input = match format.to_ascii_lowercase().as_str() {
        "ods" | "odt" | "odp" => build_sample_ods(),
        _ => build_sample_docx(),
    };
    let opts = WatermarkOptions::new(text).map_err(|e| format!("{e:#}"))?;
    let out = watermark_bytes(&input, &opts).map_err(|e| format!("{e:#}"))?;
    let hits = extract_watermarks_bytes(&out).map_err(|e| format!("{e:#}"))?;
    let ok = verify_all_equal(&hits, text);
    Ok(RoundtripResult { format: format.to_string(), text: text.to_string(), hits, ok })
}

/// Default output path: `<stem>-wm.<ext>` next to the input.
pub fn default_output(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let ext = input
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    input
        .parent()
        .map(|p| p.join(format!("{stem}-wm{ext}")))
        .unwrap_or_else(|| PathBuf::from(format!("{stem}-wm{ext}")))
}

pub fn build_sample_docx() -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let o = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        w.start_file("[Content_Types].xml", o).unwrap();
        w.write_all(br#"<?xml version="1.0"?><Types xmlns="http://schemas"><Override PartName="/word/document.xml"/></Types>"#).unwrap();
        w.start_file("_rels/.rels", o).unwrap();
        w.write_all(br#"<Relationships xmlns="http://rels"/>"#)
            .unwrap();
        w.start_file("word/document.xml", o).unwrap();
        w.write_all(br#"<w:document xmlns:w="urn:w"><w:body><w:p><w:r><w:t>demo</w:t></w:r></w:p></w:body></w:document>"#).unwrap();
        w.start_file("word/styles.xml", o).unwrap();
        w.write_all(br#"<w:styles xmlns:w="urn:w"/>"#).unwrap();
        w.finish().unwrap();
    }
    buf.into_inner()
}

pub fn build_sample_ods() -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let stored = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);
        w.start_file("mimetype", stored).unwrap();
        w.write_all(b"application/vnd.oasis.opendocument.spreadsheet")
            .unwrap();
        let o = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        w.start_file("content.xml", o).unwrap();
        w.write_all(br#"<?xml version="1.0"?><office:document-content xmlns:office="urn:oasis"><office:body/></office:document-content>"#).unwrap();
        w.start_file("styles.xml", o).unwrap();
        w.write_all(br#"<office:document-styles xmlns:office="urn:oasis"/>"#)
            .unwrap();
        w.start_file("META-INF/manifest.xml", o).unwrap();
        w.write_all(br#"<manifest:manifest xmlns:manifest="urn:manifest"/>"#)
            .unwrap();
        w.finish().unwrap();
    }
    buf.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_validation_mirrors_lib() {
        assert!(validate_text("enc-001").is_ok());
        assert!(validate_text("").is_err());
        assert!(validate_text(&"x".repeat(5000)).is_err());
        assert!(validate_text("bad\x07bell").is_err());
    }

    #[test]
    fn roundtrip_ok_both_samples() {
        for fmt in ["docx", "ods"] {
            let r = op_roundtrip(fmt, "enc-demo-001").unwrap();
            assert!(r.ok, "{fmt}");
            assert!(!r.hits.is_empty());
        }
    }

    #[test]
    fn roundtrip_empty_text_rejected() {
        assert!(op_roundtrip("docx", "").is_err());
    }

    #[test]
    fn encode_decode_verify_file_cycle() {
        let dir = std::env::temp_dir().join(format!("owm-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.docx");
        std::fs::write(&input, build_sample_docx()).unwrap();
        let output = dir.join("out.docx");

        let enc = op_encode(&input, &output, "file-cycle-1").unwrap();
        assert!(enc.watermarked_parts > 0);

        let dec = op_decode(&output).unwrap();
        assert_eq!(dec.common.as_deref(), Some("file-cycle-1"));

        let ver = op_verify(&output).unwrap();
        assert!(ver.ok);
        assert_eq!(ver.missing, 0);
        assert_eq!(ver.matched, ver.total_xml);

        // Tamper one part → verify must fail.
        let mut raw = std::fs::read(&output).unwrap();
        let needle = b"file-cycle-1";
        if let Some(pos) = raw.windows(needle.len()).position(|w| w == needle) {
            raw[pos] = b'X';
            let tampered = dir.join("tampered.docx");
            std::fs::write(&tampered, &raw).unwrap();
            let ver2 = op_verify(&tampered).unwrap();
            assert!(!ver2.ok);
        }

        // Plain (unmarked) file → decode errors like the CLI's exit-1.
        let plain = dir.join("plain.docx");
        std::fs::write(&plain, build_sample_docx()).unwrap();
        assert!(op_decode(&plain).is_err());
        // ...but verify reports not-ok rather than erroring.
        let ver3 = op_verify(&plain).unwrap();
        assert!(!ver3.ok);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_output_sibling() {
        let p = Path::new("/tmp/report.docx");
        assert_eq!(default_output(p), PathBuf::from("/tmp/report-wm.docx"));
        // unused import guard
        let mut sink = Vec::new();
        sink.write_all(b"x").unwrap();
    }
}
