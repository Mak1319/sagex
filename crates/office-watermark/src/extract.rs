use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;
use zip::ZipArchive;

use crate::error::{Error, Result};
use crate::options::WM_ATTR;
use crate::zipio::{check_magic, is_xml_entry};

/// One decoded watermark found in a single embedded XML part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatermarkHit {
    /// Zip entry name, e.g. `word/document.xml`.
    pub entry: String,
    /// Decoded `sagex:wm` text.
    pub value: String,
}

/// Decode on demand: list every `sagex:wm` found in the container.
///
/// Streams entry-by-entry with constant memory; only the root start-tag of
/// each XML part is parsed. Parts without the attribute are skipped (see
/// [`verify_all_equal`] for strict checking).
pub fn extract_watermarks_stream<R: Read + Seek>(reader: R) -> Result<Vec<WatermarkHit>> {
    let mut archive = ZipArchive::new(reader).map_err(|e| match e {
        zip::result::ZipError::InvalidArchive(msg) => Error::UnsupportedFormat(msg.to_string()),
        other => Error::Zip(other),
    })?;

    if archive.is_empty() {
        return Err(Error::EmptyInput);
    }

    let mut hits = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_owned();
        if name == "mimetype" || !is_xml_entry(&name) {
            continue;
        }
        // Empty parts carry no root and therefore no watermark (see zipio).
        if file.size() == 0 {
            continue;
        }
        let input = BufReader::with_capacity(64 * 1024, &mut file);
        if let Some(value) = extract_root_wm(input, &name)? {
            hits.push(WatermarkHit { entry: name, value });
        }
    }
    Ok(hits)
}

/// Decode from raw bytes.
pub fn extract_watermarks_bytes(input: &[u8]) -> Result<Vec<WatermarkHit>> {
    if input.is_empty() {
        return Err(Error::EmptyInput);
    }
    check_magic(&input[..input.len().min(8)])?;
    extract_watermarks_stream(Cursor::new(input))
}

/// Decode from a file on disk (streaming).
pub fn extract_watermarks_file(path: &Path) -> Result<Vec<WatermarkHit>> {
    let mut header = [0u8; 8];
    {
        let mut f = File::open(path)?;
        let _ = std::io::Read::read(&mut f, &mut header);
    }
    check_magic(&header)?;
    extract_watermarks_stream(File::open(path)?)
}

/// Strict check: every hit equals `expected` (and at least one hit exists).
pub fn verify_all_equal(hits: &[WatermarkHit], expected: &str) -> bool {
    !hits.is_empty() && hits.iter().all(|h| h.value == expected)
}

/// Parse only the root start-tag of one XML part, returning the decoded
/// `sagex:wm` value if present.
fn extract_root_wm<R: std::io::BufRead>(reader: R, entry: &str) -> Result<Option<String>> {
    let mut reader = Reader::from_reader(reader);
    reader.config_mut().trim_text(false);
    reader.config_mut().expand_empty_elements = false;
    reader.config_mut().check_end_names = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                for attr_res in e.attributes() {
                    let attr = attr_res.map_err(|e| Error::InvalidXml {
                        entry: entry.to_owned(),
                        reason: format!("bad attribute: {e}"),
                    })?;
                    if attr.key.as_ref() == WM_ATTR.as_bytes() {
                        let decoded = attr.unescape_value().map_err(|source| Error::Xml {
                            entry: entry.to_owned(),
                            source,
                        })?;
                        return Ok(Some(decoded.into_owned()));
                    }
                }
                return Ok(None);
            }
            Ok(Event::Eof) => {
                return Err(Error::InvalidXml {
                    entry: entry.to_owned(),
                    reason: "no root element found".into(),
                });
            }
            Ok(_) => {} // Decl, Comment, PI, DocType, Text before root
            Err(source) => {
                return Err(Error::Xml {
                    entry: entry.to_owned(),
                    source,
                });
            }
        }
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::WatermarkOptions;
    use crate::watermark_bytes;
    use std::io::Write;

    fn watermarked_doc(value: &str) -> Vec<u8> {
        let opts = WatermarkOptions::new(value).unwrap();
        let input = crate::xml_inject::inject_slice(b"<r/>", "x.xml", &opts).unwrap();
        let _ = input;
        // Build a minimal container with two XML parts.
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("[Content_Types].xml", o).unwrap();
            w.write_all(br#"<Types xmlns="http://schemas"/>"#).unwrap();
            w.start_file("word/document.xml", o).unwrap();
            w.write_all(b"<w:document xmlns:w='urn:w'/>").unwrap();
            w.finish().unwrap();
        }
        watermark_bytes(&buf.into_inner(), &opts).unwrap()
    }

    #[test]
    fn extracts_every_part() {
        let data = watermarked_doc("enc-42");
        let hits = extract_watermarks_bytes(&data).unwrap();
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert!(hits.iter().all(|h| h.value == "enc-42"));
    }

    #[test]
    fn missing_attribute_yields_no_hits() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("word/document.xml", o).unwrap();
            w.write_all(b"<w:document/>").unwrap();
            w.finish().unwrap();
        }
        let hits = extract_watermarks_bytes(&buf.into_inner()).unwrap();
        assert!(hits.is_empty());
        assert!(!verify_all_equal(&hits, "x"));
    }

    #[test]
    fn escaped_values_roundtrip() {
        let data = watermarked_doc("a&b<>\"'c");
        let hits = extract_watermarks_bytes(&data).unwrap();
        assert!(hits.iter().all(|h| h.value == "a&b<>\"'c"), "{hits:?}");
    }
}
