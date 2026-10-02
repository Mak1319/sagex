use std::{env, fs, path::PathBuf};

use office_l2wm::{FontWatermark, ZipArchive};

fn file_to_bytes(path: impl Into<PathBuf>) -> std::io::Result<Vec<u8>> {
    fs::read(path.into())
}

fn invalid(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, msg)
}

/// Example-local codec helpers (not part of the library API).
struct CliCodec;

impl CliCodec {
    fn random_bytes(n: usize) -> Vec<u8> {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ (d.as_secs().wrapping_mul(0x9e3779b1)))
            .unwrap_or(0x243f6a88);
        let mut s = t | 1;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s >> 11) as u8
            })
            .collect()
    }

    fn bytes_to_hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn parse_size(args: &[String]) -> std::io::Result<usize> {
        let mut size = 16usize;
        if let Some(i) = args.iter().position(|a| a == "--size") {
            let v = args
                .get(i + 1)
                .ok_or_else(|| invalid("missing value for --size"))?;
            size = v
                .parse()
                .map_err(|_| invalid("--size must be one of 4,8,16,32,64,128,256,512"))?;
        }
        match size {
            4 | 8 | 16 | 32 | 64 | 128 | 256 | 512 => Ok(size),
            _ => Err(invalid("--size must be one of 4,8,16,32,64,128,256,512")),
        }
    }
}

fn cmd_encode(args: &[String]) -> std::io::Result<()> {
    let zin = args
        .get(2)
        .ok_or_else(|| invalid("usage: main encode <zip-in> <zip-out> [--size N]"))?;
    let zout = args
        .get(3)
        .ok_or_else(|| invalid("usage: main encode <zip-in> <zip-out> [--size N]"))?;
    let size = CliCodec::parse_size(args)?;
    let bytes = file_to_bytes(zin)?;
    let payload = CliCodec::random_bytes(size);
    let archive = ZipArchive::open(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}")))?;
    let map_err = |e: office_l2wm::Error| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}"))
    };
    let wm = FontWatermark::new(payload.clone(), archive.container_kind()).map_err(map_err)?;
    println!("font: {} ({})", wm.path, wm.face);
    let stamped = archive.embed_font_watermark(&bytes, &[wm]).map_err(map_err)?;
    fs::write(zout, stamped)?;
    println!("payload_hex: {}", CliCodec::bytes_to_hex(&payload));
    Ok(())
}

fn cmd_decode(args: &[String]) -> std::io::Result<()> {
    let path = args.get(2).ok_or_else(|| invalid("usage: main decode <zip> [--json] [--out <file>]"))?;
    let want_json = args.iter().any(|a| a == "--json");
    let bytes = file_to_bytes(path)?;
    let archive = ZipArchive::open(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}")))?;
    // font channel: *.odttf with outline-encoded payload
    let mut font_keys: Vec<String> = Vec::new();
    for f in archive.files() {
        let n = f.file_name_str();
        if n.ends_with(".xml") {
            if let Ok(l) = f.read_local(&bytes) {
                if let Ok(raw) = l.decoded_data() {
                    let s = String::from_utf8_lossy(&raw);
                    let mut k = 0;
                    while let Some(p) = s[k..].find("fontKey=\"") {
                        let a = k + p + 9;
                        if let Some(e) = s[a..].find('"') {
                            font_keys.push(s[a..a + e].to_string());
                            k = a + e;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }
    let mut font_payloads: Vec<(String, Vec<u8>)> = Vec::new();
    for f in archive.files() {
        let n = f.file_name_str();
        if n.ends_with(".odttf") {
            if let Ok(l) = f.read_local(&bytes) {
                if let Ok(raw) = l.decoded_data() {
                    let mut got = None;
                    for k in &font_keys {
                        if let Ok(p) = FontWatermark::payload_from_font(Some(k), &raw) {
                            got = Some(p);
                            break;
                        }
                    }
                    if got.is_none() {
                        got = FontWatermark::payload_from_font(None, &raw).ok();
                    }
                    if let Some(p) = got {
                        font_payloads.push((n, p));
                    }
                }
            }
        }
    }
    if font_payloads.is_empty() {
        println!("no watermark found");
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--out") {
        let out = args.get(i + 1).ok_or_else(|| invalid("missing value for --out"))?;
        for (k, (n, p)) in font_payloads.iter().enumerate() {
            let pth = if font_payloads.len() == 1 {
                out.to_string()
            } else {
                format!("{out}.font-{k}")
            };
            fs::write(&pth, p)?;
            println!("wrote {} ({} bytes, {})", pth, p.len(), n);
        }
        return Ok(());
    }
    if want_json {
        let items: Vec<String> = font_payloads
            .iter()
            .map(|(n, p)| format!("{{\"part\": \"{n}\", \"payload\": \"{}\"}}", CliCodec::bytes_to_hex(p)))
            .collect();
        println!("{{\"font_payloads\": [{}]}}", items.join(", "));
    } else {
        for (n, p) in &font_payloads {
            println!("font {n}: {}", CliCodec::bytes_to_hex(p));
        }
    }
    Ok(())
}

pub fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("encode") => return cmd_encode(&args),
        Some("decode") => return cmd_decode(&args),
        _ => {}
    }
    let path = args.get(1).ok_or_else(|| {
        invalid("usage: main <file> [--json] | main encode <zip-in> <zip-out> [--size N] | main decode <zip> [--json] [--out <file>]")
    })?;
    let want_json = args.iter().any(|a| a == "--json");
    let bytes = file_to_bytes(path)?;

    #[cfg(feature = "tree")]
    {
        let tree = office_l2wm::tree::TreeBuilder::new(&bytes)
            .build()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}")))?;
        if want_json {
            let s = serde_json::to_string_pretty(&tree).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
            })?;
            println!("{s}");
        } else {
            tree.print_text(0);
        }
        return Ok(());
    }

    #[cfg(not(feature = "tree"))]
    {
        let _ = want_json;
        let archive = ZipArchive::open(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}")))?;
        println!("files: {}", archive.files().len());
        Ok(())
    }
}
