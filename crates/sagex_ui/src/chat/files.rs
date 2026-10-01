//! Local file pipeline (offline-first): validate → copy to vault with
//! progress → attachment records. No network involved; the server upload
//! endpoints land later. The GPUI foreground thread never touches disk:
//! callers run [`ingest_paths`] on a background executor.

use std::path::{Path, PathBuf};

/// Max single-file size: 100 MB (validated before any copy starts).
pub const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
/// Max files accepted from one drop / picker session.
pub const MAX_DROP_FILES: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
    Image,
    Audio,
    Pdf,
    Text,
    Archive,
    Other,
}

impl FileKind {
    pub fn label(self) -> &'static str {
        match self {
            FileKind::Image => "Photo",
            FileKind::Audio => "Audio",
            FileKind::Pdf => "PDF",
            FileKind::Text => "Text",
            FileKind::Archive => "Archive",
            FileKind::Other => "File",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            FileKind::Image => "icons/image.svg",
            FileKind::Audio => "icons/headphones.svg",
            FileKind::Pdf => "icons/file-text.svg",
            FileKind::Text => "icons/file-text.svg",
            FileKind::Archive => "icons/archive.svg",
            FileKind::Other => "icons/paperclip.svg",
        }
    }
}

/// A file resting in the app vault, ready to send or already sent.
#[derive(Clone, Debug)]
pub struct Attachment {
    /// Vault location (always inside the app data dir).
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub kind: FileKind,
    /// Locked-mode marker (UI only for now — see `lock_bytes` seam).
    pub locked: bool,
}

/// Staged (pre-send) file in the composer tray.
#[derive(Clone, Debug)]
pub struct StagedFile {
    pub id: usize,
    pub name: String,
    pub size: u64,
    pub kind: FileKind,
    pub vault_path: Option<PathBuf>,
    pub progress: f32,
    pub error: Option<String>,
}

impl StagedFile {
    pub fn done(&self) -> bool {
        self.vault_path.is_some() && self.error.is_none()
    }
}

pub fn fmt_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn data_root() -> PathBuf {
    std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|h| PathBuf::from(h).join(".local").join("share"))
        })
        .unwrap_or_else(std::env::temp_dir)
        .join("sagex_ui")
}

/// Vault dir for a room key (local chat id or server hex). Created lazily.
pub fn vault_dir(room_key: &str) -> PathBuf {
    let safe: String = room_key
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .take(64)
        .collect();
    data_root().join("files").join(safe)
}

/// Kind by magic bytes first, extension fallback.
pub fn detect_kind(name: &str, head: &[u8]) -> FileKind {
    if head.starts_with(&[0x89, b'P', b'N', b'G'])
        || head.starts_with(&[0xFF, 0xD8, 0xFF])
        || head.starts_with(b"GIF8")
        || (head.starts_with(b"RIFF") && head.len() > 11 && &head[8..12] == b"WEBP")
        || head.starts_with(b"BM")
    {
        return FileKind::Image;
    }
    if head.starts_with(b"%PDF") {
        return FileKind::Pdf;
    }
    if head.starts_with(&[0x50, 0x4B, 0x03, 0x04]) || head.starts_with(&[0x50, 0x4B, 0x05, 0x06]) {
        return FileKind::Archive;
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" => FileKind::Image,
        "mp3" | "wav" | "ogg" | "oga" | "m4a" | "flac" | "opus" => FileKind::Audio,
        "pdf" => FileKind::Pdf,
        "txt" | "md" | "log" | "json" | "toml" | "yaml" | "yml" | "csv" | "rs" | "py" | "js"
        | "ts" | "html" | "css" => FileKind::Text,
        "zip" | "tar" | "gz" | "bz2" | "xz" | "rar" | "7z" => FileKind::Archive,
        _ => FileKind::Other,
    }
}

/// Validate one candidate path. Returns (name, size, kind) or a message.
pub fn inspect(path: &Path) -> Result<(String, u64, FileKind), String> {
    let meta = std::fs::metadata(path).map_err(|_| "Cannot read file.".to_string())?;
    if meta.is_dir() {
        return Err("Folders can't be sent yet.".to_string());
    }
    let size = meta.len();
    if size == 0 {
        return Err("Empty files can't be sent.".to_string());
    }
    if size > MAX_FILE_BYTES {
        return Err(format!("Too large ({} — 100 MB max).", fmt_size(size)));
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let mut head = [0u8; 12];
    if let Ok(mut f) = std::fs::File::open(path) {
        use std::io::Read;
        let _ = f.read(&mut head);
    }
    let kind = detect_kind(&name, &head);
    Ok((name, size, kind))
}

/// Chunked copy with progress (0.0–1.0). Blocking — run off-thread.
pub fn copy_with_progress(
    src: &Path,
    dst: &Path,
    total: u64,
    mut on_progress: impl FnMut(f32),
) -> std::io::Result<()> {
    use std::io::{Read, Write};
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut r = std::fs::File::open(src)?;
    let mut w = std::fs::File::create(dst)?;
    let mut buf = [0u8; 256 * 1024];
    let mut copied: u64 = 0;
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        w.write_all(&buf[..n])?;
        copied += n as u64;
        if total > 0 {
            on_progress((copied as f32 / total as f32).min(1.0));
        }
    }
    on_progress(1.0);
    Ok(())
}

/// Unique vault destination for `name` (stem + counter on clash).
pub fn vault_dest(room_key: &str, name: &str) -> PathBuf {
    let dir = vault_dir(room_key);
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let mut candidate = dir.join(format!("{stem}{ext}"));
    let mut n = 1u32;
    while candidate.exists() {
        n += 1;
        candidate = dir.join(format!("{stem} ({n}){ext}"));
    }
    candidate
}

// ---------------------------------------------------------------------------
// Locked-mode seam (UI only for now).
//
// When the real scheme lands, this is the single choke point: staged bytes
// pass through here on the way into the vault, and vault bytes pass through
// the inverse on open. Today it is intentionally the identity function.
// ---------------------------------------------------------------------------
#[allow(dead_code)]
pub fn lock_bytes(data: Vec<u8>, _passphrase: &str) -> Vec<u8> {
    // TODO(locked-mode): AES-256-GCM per the approved scheme.
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_format() {
        assert_eq!(fmt_size(0), "0 B");
        assert_eq!(fmt_size(999), "999 B");
        assert_eq!(fmt_size(2048), "2 KB");
        assert_eq!(fmt_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn kind_by_magic() {
        assert_eq!(
            detect_kind("a.bin", &[0x89, b'P', b'N', b'G']),
            FileKind::Image
        );
        assert_eq!(detect_kind("a.bin", b"%PDF-1.7"), FileKind::Pdf);
        assert_eq!(detect_kind("song.mp3", &[0; 12]), FileKind::Audio);
        assert_eq!(detect_kind("notes.txt", &[0; 12]), FileKind::Text);
        assert_eq!(
            detect_kind("x.zip", &[0x50, 0x4B, 0x03, 0x04]),
            FileKind::Archive
        );
        assert_eq!(detect_kind("blob.xyz", &[0; 12]), FileKind::Other);
    }

    #[test]
    fn inspect_rejects_dirs_empties() {
        let dir = std::env::temp_dir();
        assert!(inspect(&dir).is_err());
        let empty = dir.join("sagex_empty_test.bin");
        std::fs::write(&empty, []).unwrap();
        assert!(inspect(&empty).is_err());
        let _ = std::fs::remove_file(&empty);
    }

    #[test]
    fn copy_roundtrip_with_progress() {
        let dir = std::env::temp_dir().join("sagex_copy_test");
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("src.bin");
        let dst = dir.join("sub").join("dst.bin");
        let data = vec![7u8; 600 * 1024];
        std::fs::write(&src, &data).unwrap();
        let mut marks = vec![];
        copy_with_progress(&src, &dst, data.len() as u64, |f| marks.push(f)).unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), data);
        assert!(!marks.is_empty());
        assert!((marks.last().unwrap() - 1.0).abs() < f32::EPSILON);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vault_dest_unique() {
        let a = vault_dest("room!!1", "photo.png");
        assert!(a.to_string_lossy().contains("room__1"));
        // lock seam is the identity until the real scheme lands
        assert_eq!(lock_bytes(vec![1, 2, 3], "pw"), vec![1, 2, 3]);
    }
}
