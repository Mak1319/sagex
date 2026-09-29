use std::io::{Cursor, Write};

use office_watermark::{watermark_bytes, WatermarkOptions};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive};

fn build_zip(entries: &[(&str, &str, CompressionMethod)]) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        for (name, content, method) in entries {
            let opts = SimpleFileOptions::default()
                .compression_method(*method)
                .unix_permissions(0o644);
            w.start_file(*name, opts).unwrap();
            w.write_all(content.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }
    buf.into_inner()
}

fn build_ods() -> Vec<u8> {
    // ODF requires mimetype first + Stored.
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let stored = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);
        w.start_file("mimetype", stored).unwrap();
        w.write_all(b"application/vnd.oasis.opendocument.spreadsheet")
            .unwrap();
        let deflated = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        for (name, content) in [
            (
                "content.xml",
                r#"<?xml version="1.0"?><office:document-content xmlns:office="urn:oasis"><office:body/></office:document-content>"#,
            ),
            (
                "styles.xml",
                r#"<office:document-styles xmlns:office="urn:oasis"/>"#,
            ),
            (
                "META-INF/manifest.xml",
                r#"<manifest:manifest xmlns:manifest="urn:manifest"/>"#,
            ),
        ] {
            w.start_file(name, deflated).unwrap();
            w.write_all(content.as_bytes()).unwrap();
        }
        // binary part must survive byte-identical
        w.start_file("Thumbnails/thumbnail.png", deflated).unwrap();
        w.write_all(&[0x89, b'P', b'N', b'G', 1, 2, 3]).unwrap();
        w.finish().unwrap();
    }
    buf.into_inner()
}

fn xml_entries_of(data: &[u8]) -> Vec<(String, String)> {
    let mut z = ZipArchive::new(Cursor::new(data)).unwrap();
    let mut out = Vec::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().to_owned();
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut f, &mut s).unwrap();
            out.push((name, s));
        }
    }
    out
}

#[test]
fn docx_every_xml_gets_sagex_wm() {
    let input = build_zip(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas"><Override PartName="/word/document.xml"/></Types>"#,
            CompressionMethod::Deflated,
        ),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://rels"/>"#,
            CompressionMethod::Deflated,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="urn:w"><w:body><w:p/></w:body></w:document>"#,
            CompressionMethod::Deflated,
        ),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="urn:w"><w:style/></w:styles>"#,
            CompressionMethod::Deflated,
        ),
    ]);
    // keep a binary part
    let opts = WatermarkOptions::new("enc-docx-123").unwrap();
    let out = watermark_bytes(&input, &opts).unwrap();

    // Re-opens as zip => CRCs valid.
    let entries = xml_entries_of(&out);
    assert_eq!(entries.len(), 4, "{entries:?}");
    for (name, content) in &entries {
        assert!(
            content.contains(r#"sagex:wm="enc-docx-123""#),
            "{name} missing watermark: {content}"
        );
        assert!(
            content.contains(r#"xmlns:sagex="https://sagex/enc""#),
            "{name} missing xmlns"
        );
    }
    // Idempotent: second pass keeps exactly one attr per file.
    let out2 = watermark_bytes(&out, &opts).unwrap();
    for (_, content) in xml_entries_of(&out2) {
        assert_eq!(content.matches("sagex:wm=").count(), 1, "{content}");
    }
}

#[test]
fn ods_mimetype_stays_first_and_stored() {
    let input = build_ods();
    let opts = WatermarkOptions::new("enc-ods").unwrap();
    let out = watermark_bytes(&input, &opts).unwrap();

    let mut z = ZipArchive::new(Cursor::new(&out)).unwrap();
    assert_eq!(z.by_index(0).unwrap().name(), "mimetype");
    assert_eq!(
        z.by_index(0).unwrap().compression(),
        CompressionMethod::Stored
    );
    // mimetype must NOT be watermarked (not XML).
    let mut mime = String::new();
    std::io::Read::read_to_string(&mut z.by_name("mimetype").unwrap(), &mut mime).unwrap();
    assert!(!mime.contains("sagex:wm"), "{mime}");

    for (name, content) in xml_entries_of(&out) {
        assert!(content.contains("sagex:wm="), "{name}: {content}");
    }
    // binary part byte-identical, CRC recalculated (read succeeds).
    let mut thumb = Vec::new();
    std::io::Read::read_to_end(
        &mut z.by_name("Thumbnails/thumbnail.png").unwrap(),
        &mut thumb,
    )
    .unwrap();
    assert_eq!(thumb, vec![0x89, b'P', b'N', b'G', 1, 2, 3]);
}

#[test]
fn binary_parts_preserved_and_overwrite_works() {
    let input = buf_into_inner_doc();
    let opts = WatermarkOptions::new("v1").unwrap();
    let out = watermark_bytes(&input, &opts).unwrap();
    let mut z = ZipArchive::new(Cursor::new(&out)).unwrap();
    let mut bin = Vec::new();
    std::io::Read::read_to_end(&mut z.by_name("word/media/image1.png").unwrap(), &mut bin).unwrap();
    assert_eq!(bin, vec![1, 2, 3, 255, 0, 9]);

    // Overwrite with v2.
    let opts2 = WatermarkOptions::new("v2").unwrap();
    let out2 = watermark_bytes(&out, &opts2).unwrap();
    let (_, doc) = xml_entries_of(&out2).pop().unwrap();
    assert!(doc.contains(r#"sagex:wm="v2""#));
    assert!(!doc.contains(r#"sagex:wm="v1""#));
}

fn buf_into_inner_doc() -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let o = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        w.start_file("word/document.xml", o).unwrap();
        w.write_all(b"<w:document xmlns:w='urn:w'/>").unwrap();
        w.start_file("word/media/image1.png", o).unwrap();
        w.write_all(&[1, 2, 3, 255, 0, 9]).unwrap();
        w.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn rejects_empty_and_ole() {
    let opts = WatermarkOptions::new("x").unwrap();
    assert!(watermark_bytes(&[], &opts).is_err());
    // OLE magic
    let ole = vec![0xD0, 0xCF, 0x11, 0xE0, 0, 0, 0, 0];
    assert!(watermark_bytes(&ole, &opts).is_err());
}

#[test]
fn empty_xml_parts_pass_through_unchanged() {
    // Regression: LibreOffice ships 0-byte Configurations2 XML files.
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let o = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        w.start_file("content.xml", o).unwrap();
        w.write_all(b"<office:document-content xmlns:office='urn:oasis'/>")
            .unwrap();
        w.start_file("Configurations2/accelerator/current.xml", o)
            .unwrap();
        w.write_all(b"").unwrap();
        w.finish().unwrap();
    }
    let opts = WatermarkOptions::new("enc-empty").unwrap();
    let out = watermark_bytes(&buf.into_inner(), &opts).unwrap();
    let mut z = ZipArchive::new(Cursor::new(&out)).unwrap();
    // Empty part survives as empty.
    let mut empty = Vec::new();
    std::io::Read::read_to_end(
        &mut z
            .by_name("Configurations2/accelerator/current.xml")
            .unwrap(),
        &mut empty,
    )
    .unwrap();
    assert!(empty.is_empty());
    // Non-empty part got the watermark.
    let mut content = String::new();
    std::io::Read::read_to_string(&mut z.by_name("content.xml").unwrap(), &mut content).unwrap();
    assert!(content.contains(r#"sagex:wm="enc-empty""#), "{content}");
    // Decode skips the empty part without error.
    let hits = office_watermark::extract_watermarks_bytes(&out).unwrap();
    assert_eq!(hits.len(), 1, "{hits:?}");
}
