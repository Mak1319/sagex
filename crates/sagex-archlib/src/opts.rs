use std::path::PathBuf;

/// Inputs for [`crate::create`]. Mirrors the `create` CLI arguments;
/// `sign_password` replaces the interactive unlock prompt.
#[derive(Debug, Clone)]
pub struct CreateOptions {
    pub input: PathBuf,
    pub out: PathBuf,
    /// Recipient public-key files; empty means no encryption.
    pub recipient_keys: Vec<PathBuf>,
    /// Signing key file. Requires `sign_password` when set.
    pub sign_key: Option<PathBuf>,
    /// Password unlocking `sign_key`. No prompting: absent means error.
    pub sign_password: Option<String>,
    pub compress: bool,
    pub ecc: bool,
    pub license: bool,
    pub license_out: Option<PathBuf>,
}

/// Outcome of [`crate::create`]. `warnings` collects the per-entry
/// skips the CLI prints; aborts surface as [`crate::ArchError`].
#[derive(Debug)]
pub struct CreateReport {
    pub out: PathBuf,
    pub license: Option<PathBuf>,
    pub entries: usize,
    pub warnings: Vec<String>,
}

/// Inputs for [`crate::restore`]. Mirrors the `restore` CLI arguments;
/// `key_password` replaces the interactive unlock prompt.
#[derive(Debug, Clone)]
pub struct RestoreOptions {
    pub input: PathBuf,
    pub out_dir: PathBuf,
    /// Private key file (only read when entries are encrypted).
    pub key: PathBuf,
    /// Password unlocking `key`. No prompting: absent when needed means error.
    pub key_password: Option<String>,
    /// Trust key file. Same optional semantics as the CLI.
    pub trust: Option<PathBuf>,
    /// External license file (required when the archive is not self-licensed).
    pub license: Option<PathBuf>,
}

/// Per-entry skip record (greedy extraction never aborts on these).
#[derive(Debug, Clone)]
pub struct SkippedEntry {
    pub entry: String,
    pub reason: String,
}

/// Signature outcome of [`crate::restore`].
#[derive(Debug, Clone)]
pub enum SigStatus {
    Absent,
    Skipped,
    Verified(String),
}

/// Outcome of [`crate::restore`].
#[derive(Debug)]
pub struct RestoreReport {
    pub ok: Vec<PathBuf>,
    pub skipped: Vec<SkippedEntry>,
    pub checksum_ok: bool,
    pub signature: SigStatus,
}
