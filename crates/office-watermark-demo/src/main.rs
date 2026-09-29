//! Demo CLI for `office-watermark`: encode, then decode on demand.
//!
//! ```sh
//! demo encode in.docx out.docx --text enc-001
//! demo decode out.docx
//! demo verify out.docx --expect enc-001
//! demo roundtrip --format docx --text enc-demo
//! ```

use std::io::{Cursor, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use office_watermark::{
    extract_watermarks_bytes, extract_watermarks_file, verify_all_equal, watermark_bytes,
    watermark_file, WatermarkOptions,
};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

#[derive(Debug, Parser)]
#[command(
    name = "office-watermark-demo",
    about = "Encode/decode sagex:wm Office watermarks"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Encode: inject watermark text into every embedded XML.
    Encode {
        /// Input docx/pptx/xlsx/ods/odt/odp file.
        input: PathBuf,
        /// Output watermarked file.
        output: PathBuf,
        /// Watermark text (stored as sagex:wm).
        #[arg(long)]
        text: String,
    },
    /// Decode on demand: list every sagex:wm found per XML part, then print
    /// the watermark value at the end.
    Decode {
        /// Watermarked file to inspect.
        file: PathBuf,
    },
    /// Verify every XML part carries one identical watermark, then print the
    /// watermark value at the end. No expected key needed.
    Verify {
        /// Watermarked file to check.
        file: PathBuf,
    },
    /// Self-contained test: build a sample, encode, decode, print.
    Roundtrip {
        /// Sample container to generate: docx or ods.
        #[arg(long, default_value = "docx")]
        format: String,
        /// Watermark text.
        #[arg(long, default_value = "enc-demo-001")]
        text: String,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Encode {
            input,
            output,
            text,
        } => {
            let opts = WatermarkOptions::new(&text)?;
            let format = watermark_file(&input, &output, &opts)?;
            println!(
                "encoded {format:?}: {} -> {}",
                input.display(),
                output.display()
            );
            let hits = extract_watermarks_file(&output)?;
            println!("watermarked {} xml parts", hits.len());
        }
        Commands::Decode { file } => {
            let hits = extract_watermarks_file(&file)?;
            if hits.is_empty() {
                println!("no sagex:wm watermarks found in {}", file.display());
                std::process::exit(1);
            }
            for h in &hits {
                println!("{} => {}", h.entry, h.value);
            }
            println!("decoded {} xml parts", hits.len());
            match common_value(&hits) {
                Some(wm) => println!("watermark: {wm:?}"),
                None => {
                    println!("multiple distinct watermarks found:");
                    let mut seen = Vec::new();
                    for h in &hits {
                        if !seen.contains(&h.value) {
                            seen.push(h.value.clone());
                        }
                    }
                    for v in &seen {
                        println!("  {v:?}");
                    }
                    std::process::exit(1);
                }
            }
        }
        Commands::Verify { file } => {
            let hits = extract_watermarks_file(&file)?;
            let total_xml = count_xml_entries(&file)?;
            let missing = total_xml.saturating_sub(hits.len());
            let reference = hits.first().map(|h| h.value.clone()).unwrap_or_default();
            for h in &hits {
                let mark = if h.value == reference { "OK " } else { "DIFF" };
                println!("[{mark}] {} => {}", h.entry, h.value);
            }
            let matched = hits.iter().filter(|h| h.value == reference).count();
            let ok = verify_all_equal(&hits, &reference) && missing == 0;
            if ok {
                println!("watermark: {reference:?}");
            }
            println!("verify: {matched}/{total_xml} xml parts match (missing wm: {missing})");
            if !ok {
                if missing > 0 {
                    println!("missing watermark in {missing} xml part(s)");
                }
                std::process::exit(1);
            }
        }
        Commands::Roundtrip { format, text } => {
            let input = match format.to_ascii_lowercase().as_str() {
                "ods" | "odt" | "odp" => build_sample_ods(),
                _ => build_sample_docx(),
            };
            let opts = WatermarkOptions::new(&text)?;
            let out = watermark_bytes(&input, &opts)?;
            let hits = extract_watermarks_bytes(&out)?;
            println!("sample format: {format}, watermark: {text:?}");
            for h in &hits {
                println!("{} => {}", h.entry, h.value);
            }
            let ok = verify_all_equal(&hits, &text);
            println!(
                "roundtrip {} ({} parts)",
                if ok { "OK" } else { "FAIL" },
                hits.len()
            );
            if !ok {
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

/// The single watermark value if every hit agrees, else `None`.
fn common_value(hits: &[office_watermark::WatermarkHit]) -> Option<String> {
    let first = hits.first()?;
    if hits.iter().all(|h| h.value == first.value) {
        Some(first.value.clone())
    } else {
        None
    }
}

/// Count watermarkable entries for strict verify reporting.
///
/// Mirrors the library rule: `mimetype`, directories and empty (0-byte) XML
/// parts cannot carry a watermark and are excluded.
fn count_xml_entries(path: &PathBuf) -> Result<usize, Box<dyn std::error::Error>> {
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

fn build_sample_docx() -> Vec<u8> {
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

fn build_sample_ods() -> Vec<u8> {
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
