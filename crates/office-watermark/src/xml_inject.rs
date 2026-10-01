use std::io::{BufRead, Write};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};

use crate::error::{Error, Result};
use crate::options::{WatermarkOptions, WM_ATTR, WM_NS, WM_XMLNS_ATTR};

/// Inject `sagex:wm` into an in-memory XML document.
///
/// Returns a new buffer with exactly one watermark attribute on the root
/// element. All other bytes/events are preserved verbatim.
pub fn inject_slice(xml: &[u8], entry: &str, opts: &WatermarkOptions) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(xml.len().saturating_add(128));
    let reader: &[u8] = xml;
    inject_stream(std::io::BufReader::new(reader), &mut out, entry, opts)?;
    Ok(out)
}

/// Streaming injector: `reader` is one XML part, `writer` receives the
/// watermarked XML. Constant memory (only the current event is buffered).
///
/// Behaviour:
/// * copies XML declaration, comments, PIs, DocType before the root verbatim
/// * on the first `Start` / `Empty` (the root), adds
///   `xmlns:sagex="https://sagex/enc"` (if missing) + `sagex:wm="value"`
/// * if `sagex:wm` already exists it is overwritten (idempotent)
/// * everything after the root is copied event-by-event
pub fn inject_stream<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    entry: &str,
    opts: &WatermarkOptions,
) -> Result<()> {
    let mut reader = Reader::from_reader(reader);
    reader.config_mut().trim_text(false);
    reader.config_mut().expand_empty_elements = false;
    reader.config_mut().check_end_names = false;

    let mut writer = Writer::new(writer);
    let mut buf = Vec::new();
    let mut root_done = false;

    // NOTE: quick-xml escapes attribute values on write, so pass the raw
    // watermark text here (pre-escaping would double-escape).
    let raw_value = opts.value().to_owned();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) if !root_done => {
                root_done = true;
                let new_elem = build_root_with_wm(&e, entry, &raw_value)?;
                writer
                    .write_event(Event::Start(new_elem))
                    .map_err(|source| Error::Xml {
                        entry: entry.to_owned(),
                        source,
                    })?;
            }
            Ok(Event::Empty(e)) if !root_done => {
                root_done = true;
                let new_elem = build_root_with_wm(&e, entry, &raw_value)?;
                writer
                    .write_event(Event::Empty(new_elem))
                    .map_err(|source| Error::Xml {
                        entry: entry.to_owned(),
                        source,
                    })?;
            }
            Ok(ev) => {
                // Pass through everything else verbatim (Decl, Comment, PI,
                // DocType, Text, End, CData, ...). Borrowed event data is
                // valid until buf is cleared below, and write_event consumes
                // it immediately, so this is safe.
                writer.write_event(ev).map_err(|source| Error::Xml {
                    entry: entry.to_owned(),
                    source,
                })?;
            }
            Err(source) => {
                return Err(Error::Xml {
                    entry: entry.to_owned(),
                    source,
                });
            }
        }
        buf.clear();
    }

    if !root_done {
        return Err(Error::InvalidXml {
            entry: entry.to_owned(),
            reason: "no root element found".into(),
        });
    }

    Ok(())
}

/// Clone the root start-tag, preserving all existing attributes, ensuring
/// `xmlns:sagex` + `sagex:wm="<escaped>"` are present (`sagex:wm` overwritten).
fn build_root_with_wm(
    orig: &BytesStart<'_>,
    entry: &str,
    raw_value: &str,
) -> Result<BytesStart<'static>> {
    let name_bytes = orig.name().as_ref().to_vec();
    let name_str = String::from_utf8(name_bytes.clone()).map_err(|_| Error::InvalidXml {
        entry: entry.to_owned(),
        reason: "root element name is not valid UTF-8".into(),
    })?;
    let mut out = BytesStart::new(name_str);

    let mut has_xmlns = false;

    for attr_res in orig.attributes() {
        let attr = attr_res.map_err(|e| Error::InvalidXml {
            entry: entry.to_owned(),
            reason: format!("bad attribute: {e}"),
        })?;
        let key = attr.key.as_ref().to_vec();
        if key == WM_ATTR.as_bytes() {
            continue; // drop old watermark, re-added below
        }
        if key == WM_XMLNS_ATTR.as_bytes() {
            has_xmlns = true;
            // Preserve existing declaration value as-is.
            let v = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
            let k = String::from_utf8_lossy(&key).into_owned();
            out.push_attribute((k.as_str(), v.as_str()));
            continue;
        }
        let k = String::from_utf8_lossy(&key).into_owned();
        let v = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
        out.push_attribute((k.as_str(), v.as_str()));
    }

    if !has_xmlns {
        out.push_attribute((WM_XMLNS_ATTR, WM_NS));
    }
    out.push_attribute((WM_ATTR, raw_value));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(v: &str) -> WatermarkOptions {
        WatermarkOptions::new(v).unwrap()
    }

    #[test]
    fn injects_on_normal_root() {
        let xml = br#"<?xml version="1.0"?><w:document xmlns:w="urn:x"><w:body/></w:document>"#;
        let out = inject_slice(xml, "word/document.xml", &opts("hello")).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains(r#"sagex:wm="hello""#), "{s}");
        assert!(s.contains(r#"xmlns:sagex="https://sagex/enc""#), "{s}");
        // original content preserved
        assert!(s.contains("<w:body/>"), "{s}");
    }

    #[test]
    fn overwrites_existing_watermark_idempotently() {
        let xml = br#"<root xmlns:sagex="https://sagex/enc" sagex:wm="old"><a/></root>"#;
        let out = inject_slice(xml, "content.xml", &opts("new")).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert_eq!(s.matches("sagex:wm=").count(), 1);
        assert!(s.contains(r#"sagex:wm="new""#), "{s}");
    }

    #[test]
    fn handles_self_closing_root() {
        let xml = br#"<Types xmlns="http://schemas"/>"#;
        let out = inject_slice(xml, "[Content_Types].xml", &opts("enc123")).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("sagex:wm="), "{s}");
    }

    #[test]
    fn escapes_special_chars() {
        let out = inject_slice(b"<r/>", "x.xml", &opts("a&b<>\"'c")).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("a&amp;b"), "{s}");
    }

    #[test]
    fn rejects_empty_xml() {
        let err = inject_slice(b"   ", "x.xml", &opts("v")).unwrap_err();
        assert!(matches!(err, Error::InvalidXml { .. }), "{err:?}");
    }
}
