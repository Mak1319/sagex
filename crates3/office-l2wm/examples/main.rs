use std::{env, fs, path::PathBuf};

use office_l2wm::{CoverKind, WatermarkKind, ZipArchive};

fn file_to_bytes(path: impl Into<PathBuf>) -> std::io::Result<Vec<u8>> {
    fs::read(path.into())
}

fn invalid(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, msg)
}

fn invalid_data(msg: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg)
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

    fn parse_cover(args: &[String]) -> std::io::Result<CoverKind> {
        let mut cover = "png";
        if let Some(i) = args.iter().position(|a| a == "--cover") {
            cover = args.get(i + 1).ok_or_else(|| invalid("missing value for --cover"))?;
        }
        CoverKind::parse(cover).map_err(|_| invalid("--cover must be png or wav"))
    }

    fn parse_rep(args: &[String]) -> usize {
        if let Some(i) = args.iter().position(|a| a == "--rep") {
            if let Some(v) = args.get(i + 1).and_then(|s| s.parse::<usize>().ok()) {
                return v.clamp(1, 5);
            }
        }
        3
    }

    fn parse_kind(args: &[String]) -> std::io::Result<Option<WatermarkKind>> {
        // legacy --dir shorthand
        if args.iter().any(|a| a == "--dir") {
            return Ok(Some(WatermarkKind::Dir));
        }
        if let Some(i) = args.iter().position(|a| a == "--kind") {
            let k = args.get(i + 1).ok_or_else(|| invalid("missing value for --kind"))?;
            if k == "all" {
                return Ok(None);
            }
            return WatermarkKind::parse(k)
                .map(Some)
                .map_err(|_| invalid("--kind must be dir|file|linked|attr|font|image|all"));
        }
        Ok(Some(WatermarkKind::Font))
    }
}

fn cmd_encode(args: &[String]) -> std::io::Result<()> {
    let zin = args.get(2).ok_or_else(|| {
        invalid("usage: main encode <zip-in> <zip-out> [--size N] [--cover png|wav] [--kind dir|file|linked|attr|font|image|all] [--rep N] [--dir]")
    })?;
    let zout = args.get(3).ok_or_else(|| {
        invalid("usage: main encode <zip-in> <zip-out> [--size N] [--cover png|wav] [--kind dir|file|linked|attr|font|image|all] [--rep N] [--dir]")
    })?;
    let size = CliCodec::parse_size(args)?;
    let bytes = file_to_bytes(zin)?;
    let payload = CliCodec::random_bytes(size);
    let archive =
        ZipArchive::open(&bytes).map_err(|e| invalid_data(format!("{e}")))?;
    let map_err = |e: office_l2wm::Error| invalid_data(format!("{e}"));
    let rep = CliCodec::parse_rep(args);
    let cover = CliCodec::parse_cover(args)?;
    let parts: Vec<String> = archive.files().iter().map(|f| f.file_name_str()).collect();
    let container = office_l2wm::ContainerKind::detect(&parts);
    let stamped = match CliCodec::parse_kind(args)? {
        None => archive.embed_all_watermarks(&bytes, payload.clone()).map_err(map_err)?,
        Some(WatermarkKind::Dir) => {
            let wm = office_l2wm::DirWatermark::new(payload.clone()).map_err(map_err)?;
            println!("dirs: {}", wm.entries.len());
            archive.embed_dir_watermark(&bytes, &[wm]).map_err(map_err)?
        }
        Some(WatermarkKind::File) => {
            let wm =
                office_l2wm::FileWatermark::new(payload.clone(), cover).map_err(map_err)?;
            println!("entry: {}", wm.path);
            archive.embed_file_watermark(&bytes, &[wm]).map_err(map_err)?
        }
        Some(WatermarkKind::Linked) => {
            let wm =
                office_l2wm::LinkedWatermark::new(payload.clone(), cover).map_err(map_err)?;
            println!("entry: {}", wm.file.path);
            println!("rels: {} {}", wm.media_rel_id, wm.custom_rel_id);
            archive.embed_linked_watermark(&bytes, &[wm]).map_err(map_err)?
        }
        Some(WatermarkKind::Attr) => {
            let wm = office_l2wm::AttrWatermark::new(payload.clone(), &parts, container, rep)
                .map_err(map_err)?;
            println!("container: {:?}", container);
            println!("attr parts: {}", wm.assignments.len());
            archive.embed_attr_watermark(&bytes, &wm).map_err(map_err)?
        }
        Some(WatermarkKind::Font) => {
            let wm =
                office_l2wm::FontWatermark::new(payload.clone(), container).map_err(map_err)?;
            println!("font: {} ({})", wm.path, wm.face);
            archive.embed_font_watermark(&bytes, &[wm]).map_err(map_err)?
        }
        Some(WatermarkKind::Image) => {
            let wm = office_l2wm::ImageWatermark::new(payload.clone()).map_err(map_err)?;
            println!("image: {} ({}x{})", wm.path, wm.width, wm.height);
            archive.embed_image_watermark(&bytes, &[wm]).map_err(map_err)?
        }
    };
    fs::write(zout, stamped)?;
    println!("payload_hex: {}", CliCodec::bytes_to_hex(&payload));
    Ok(())
}

fn cmd_decode(args: &[String]) -> std::io::Result<()> {
    let path = args
        .get(2)
        .ok_or_else(|| invalid("usage: main decode <zip> [--json] [--out <file>] [--verify-links]"))?;
    let want_json = args.iter().any(|a| a == "--json");
    let verify = args.iter().any(|a| a == "--verify-links");
    let bytes = file_to_bytes(path)?;
    let archive =
        ZipArchive::open(&bytes).map_err(|e| invalid_data(format!("{e}")))?;
    let hits = archive.extract_watermarks(&bytes);
    if hits.is_empty() {
        println!("no watermark found");
        return Ok(());
    }
    if verify {
        // link + reference verification per hit
        let mut rels_text = String::new();
        let mut ct_text = String::new();
        for f in archive.files() {
            let n = f.file_name_str();
            if n.ends_with(".rels") || n == "[Content_Types].xml" {
                if let Ok(l) = f.read_local(&bytes) {
                    if let Ok(raw) = l.decoded_data() {
                        let s = String::from_utf8_lossy(&raw).into_owned();
                        if n.ends_with(".rels") {
                            rels_text.push_str(&s);
                        } else {
                            ct_text = s;
                        }
                    }
                }
            }
        }
        for h in &hits {
            let linked = rels_text.contains(&h.part) || ct_text.contains(&h.part);
            println!(
                "verify {} {}: {}",
                h.kind,
                h.part,
                if linked || h.kind == "dir" || h.kind == "attr" {
                    "present"
                } else {
                    "UNLINKED"
                }
            );
        }
    }
    if let Some(i) = args.iter().position(|a| a == "--out") {
        let out = args.get(i + 1).ok_or_else(|| invalid("missing value for --out"))?;
        for (k, h) in hits.iter().enumerate() {
            let pth = if hits.len() == 1 {
                out.to_string()
            } else {
                format!("{out}.{k}")
            };
            fs::write(&pth, &h.payload)?;
            println!("wrote {} ({} bytes, {} {})", pth, h.payload.len(), h.kind, h.part);
        }
        return Ok(());
    }
    if want_json {
        let items: Vec<String> = hits
            .iter()
            .map(|h| {
                format!(
                    "{{\"kind\": \"{}\", \"part\": \"{}\", \"face\": \"{}\", \"renamed\": {}, \"payload\": \"{}\"}}",
                    h.kind,
                    h.part,
                    h.face,
                    h.renamed,
                    CliCodec::bytes_to_hex(&h.payload)
                )
            })
            .collect();
        println!("{{\"watermarks\": [{}]}}", items.join(", "));
    } else {
        for h in &hits {
            let mut line = format!(
                "{} {}: {}",
                h.kind,
                if h.face.is_empty() { h.part.clone() } else { format!("{} ({})", h.part, h.face) },
                CliCodec::bytes_to_hex(&h.payload)
            );
            if h.renamed {
                line.push_str(" [renamed]");
            }
            println!("{line}");
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
        invalid("usage: main <file> [--json] | main encode <zip-in> <zip-out> [opts] | main decode <zip> [--json] [--out <file>]")
    })?;
    let want_json = args.iter().any(|a| a == "--json");
    let bytes = file_to_bytes(path)?;

    #[cfg(feature = "tree")]
    {
        let tree = office_l2wm::tree::TreeBuilder::new(&bytes)
            .build()
            .map_err(|e| invalid_data(format!("{e}")))?;
        if want_json {
            let s = serde_json::to_string_pretty(&tree)
                .map_err(|e| invalid_data(e.to_string()))?;
            println!("{s}");
        } else {
            tree.print_text(0);
        }
        return Ok(());
    }

    #[cfg(not(feature = "tree"))]
    {
        let _ = want_json;
        let archive =
            ZipArchive::open(&bytes).map_err(|e| invalid_data(format!("{e}")))?;
        println!("files: {}", archive.files().len());
        Ok(())
    }
}
