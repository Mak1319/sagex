/// Failure modes for headless archive operations.
///
/// Abort paths in the CLI (`println!` + `return`) map to these variants;
/// per-entry skips are collected in reports instead of erroring.
#[derive(Debug)]
pub enum ArchError {
    /// Filesystem failure (open/read/write/seek/permissions).
    Io(std::io::Error),
    /// Malformed archive structure (bad magic, version, offsets, sizes).
    Invalid(String),
    /// No EOCD magic found: not an archive.
    NotAnArchive,
    /// Encrypted archive where no recipient could be wrapped.
    NoRecipients,
    /// Signature required but unavailable (large unsigned archive,
    /// required verification without `--trust`, unsigned checksum mismatch).
    SignatureRequired(String),
    /// Requested signature verification failed.
    VerificationFailed(String),
    /// A required password was not supplied in the options.
    MissingPassword(&'static str),
    /// Key file unreadable or unparsable.
    KeyFile(String),
    /// Cryptographic operation failed (decrypt, KEM, digest excluded).
    Crypto(String),
    /// No DEK entry for this identity.
    NoLicenseFor(String),
    /// Encrypted archive with external licensing but no license file given.
    LicenseRequired,
}

impl From<std::io::Error> for ArchError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
