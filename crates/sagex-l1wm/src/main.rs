//! Hands-on CLI for the `sagex-l1wm` watermark library.
//!
//! ```sh
//! sagex-l1wm info -i report.docx
//! sagex-l1wm embed -i report.docx -o wm.docx --random uuid
//! sagex-l1wm extract -i wm.docx
//! ```

use clap::{Args, Parser, Subcommand};
use sagex_l1wm::{Watermark, embed, inspect, locate};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sagex-l1wm", about = "Invisible forensic watermark for ZIP office files")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Embed a watermark (before 2nd LFH + before central directory).
    Embed {
        #[arg(short, long, help = "Input docx/xlsx/pptx/odt/ods/odp/zip")]
        input: PathBuf,
        #[arg(short, long, help = "Output file")]
        output: PathBuf,
        #[command(flatten)]
        wm: WmSource,
    },
    /// Scan a file and trace out its watermark.
    Extract {
        #[arg(short, long, help = "Possibly watermarked file")]
        input: PathBuf,
    },
    /// Show archive layout and where the watermark would go.
    Info {
        #[arg(short, long)]
        input: PathBuf,
    },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct WmSource {
    #[arg(long, help = "UUID, hyphenated or bare 32-hex")]
    uuid: Option<String>,
    #[arg(long, help = "Raw payload as 512 hex chars (256 bytes)")]
    hex: Option<String>,
    #[arg(long, value_enum, help = "Auto-generate from OS randomness")]
    random: Option<RandomKind>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum RandomKind {
    Uuid,
    Raw,
}

fn hex_val(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("bad hex char {b:?}")),
    }
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    let b = s.as_bytes();
    if b.len() % 2 != 0 {
        return Err("hex string has odd length".to_owned());
    }
    b.chunks_exact(2)
        .map(|c| Ok(hex_val(c[0])? << 4 | hex_val(c[1])?))
        .collect()
}

fn parse_uuid(s: &str) -> Result<[u8; 16], String> {
    let clean: String = s.chars().filter(|&c| c != '-').collect();
    let v = hex_decode(&clean)?;
    if v.len() != 16 {
        return Err(format!("uuid must be 32 hex chars, got {}", clean.len()));
    }
    let mut id = [0u8; 16];
    id.copy_from_slice(&v);
    Ok(id)
}

fn parse_raw_hex(s: &str) -> Result<[u8; 256], String> {
    let v = hex_decode(s)?;
    if v.len() != 256 {
        return Err(format!("raw payload must be 512 hex chars, got {}", s.len()));
    }
    let mut arr = [0u8; 256];
    arr.copy_from_slice(&v);
    Ok(arr)
}

fn hex_encode(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 0xF) as usize] as char);
    }
    s
}

fn format_uuid(id: &[u8; 16]) -> String {
    let h = hex_encode(id);
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).map_err(|e| format!("rng failed: {e}"))?;
    Ok(buf)
}

fn build_watermark(src: &WmSource) -> Result<(Watermark, String), String> {
    if let Some(u) = &src.uuid {
        let id = parse_uuid(u)?;
        return Ok((Watermark::Uuid(id), format!("uuid {}", format_uuid(&id))));
    }
    if let Some(h) = &src.hex {
        let arr = parse_raw_hex(h)?;
        return Ok((Watermark::Raw(arr), format!("raw {}", hex_encode(&arr))));
    }
    match src.random {
        Some(RandomKind::Uuid) => {
            let id = random_bytes::<16>()?;
            Ok((Watermark::Uuid(id), format!("uuid {}", format_uuid(&id))))
        }
        Some(RandomKind::Raw) => {
            let arr = random_bytes::<256>()?;
            Ok((Watermark::Raw(arr), format!("raw {}", hex_encode(&arr))))
        }
        None => Err("one of --uuid, --hex, --random is required".to_owned()),
    }
}

fn run() -> Result<i32, String> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Embed { input, output, wm } => {
            let (watermark, label) = build_watermark(&wm)?;
            let data = std::fs::read(&input).map_err(|e| format!("read {}: {e}", input.display()))?;
            let before = inspect(&data).map_err(|e| format!("parse {}: {e}", input.display()))?;
            let n = watermark.wire_len();
            let out = embed(&data, &watermark).map_err(|e| format!("embed: {e}"))?;
            std::fs::write(&output, &out).map_err(|e| format!("write {}: {e}", output.display()))?;
            let after = inspect(&out).map_err(|e| format!("re-parse: {e}"))?;
            println!("watermark : {label} ({} bytes on wire)", watermark.wire_len());
            println!("copy #1   : @{}{}", before.insert_point, match &before.second_name {
                Some(name) => format!(" (before 2nd LFH {name:?})"),
                None => " (back-to-back pre-CD: < 2 entries)".to_owned(),
            });
            println!("copy #2   : @{} (before central directory)", after.cd_off - n as u64);
            println!("cd offset : {} -> {} (+{})", before.cd_off, after.cd_off, n * 2);
            println!("wrote     : {} ({} bytes)", output.display(), out.len());
            Ok(0)
        }
        Cmd::Extract { input } => {
            let data = std::fs::read(&input).map_err(|e| format!("read {}: {e}", input.display()))?;
            let found = locate(&data).map_err(|e| format!("extract: {e}"))?;
            let wm = found.watermark;
            let n = wm.wire_len();
            let (label, detail) = match &wm {
                Watermark::Uuid(id) => ("uuid".to_owned(), format_uuid(id)),
                Watermark::Raw(arr) => ("raw".to_owned(), hex_encode(arr)),
            };
            println!("variant   : {label}");
            println!("payload   : {detail}");
            println!("wire      : {n} bytes (magic + postcard)");
            println!("copy #1   : @{}", found.copy_a);
            println!("copy #2   : @{}", found.copy_b);
            println!("copies    : match ok");
            Ok(0)
        }
        Cmd::Info { input } => {
            let data = std::fs::read(&input).map_err(|e| format!("read {}: {e}", input.display()))?;
            let info = inspect(&data).map_err(|e| format!("parse {}: {e}", input.display()))?;
            println!("file      : {} ({} bytes)", input.display(), data.len());
            println!("entries   : {}", info.count);
            println!("cd        : offset={} size={}", info.cd_off, info.cd_size);
            println!("zip64     : {}", if info.is_zip64 { "yes" } else { "no" });
            match &info.second_name {
                Some(name) => println!("insert    : @{} (before 2nd LFH {name:?})", info.insert_point),
                None => println!("insert    : @{} (back-to-back pre-CD: < 2 entries)", info.insert_point),
            }
            Ok(0)
        }
    }
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
