//! Encrypted on-disk vault for credentials, identity keys, and the device
//! certificate. Nothing here is ever plaintext: the session blob is
//! AES-GCM-encrypted with a password KDF (same `sagex-crypto` primitive as
//! `.prv` files), and the keypair itself lives in `sagex-format` files whose
//! secrets are password-encapsulated.
//!
//! Layout under the vault root (default
//! `$XDG_DATA_HOME/sagex_ui/vault/`, 0600 files on unix):
//! `session.enc`, `identity.prv`, `identity.pub`, `device.pem`,
//! `device.json {identity, serial}`.

use binrw::{BinRead, BinWrite};
use sagex_crypto::aes::AESHandler;
use sagex_format::format::{PrivateExternal, PublicFileFormatExternal};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use super::session::StoredSession;

const SESSION_FILE: &str = "session.enc";
const PRV_FILE: &str = "identity.prv";
const PUB_FILE: &str = "identity.pub";
const CERT_PEM_FILE: &str = "device.pem";
const CERT_META_FILE: &str = "device.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertMeta {
    pub identity: String,
    pub serial: u64,
}

#[derive(Debug)]
pub enum VaultError {
    Io(String),
    Format(String),
    Crypto(String),
    Decode(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "vault I/O: {e}"),
            Self::Format(e) => write!(f, "vault format: {e}"),
            Self::Crypto(e) => write!(f, "vault crypto: {e}"),
            Self::Decode(e) => write!(f, "vault decode: {e}"),
        }
    }
}

impl std::error::Error for VaultError {}

pub type VaultResult<T> = Result<T, VaultError>;

pub fn default_vault_dir() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|h| PathBuf::from(h).join(".local").join("share"))
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join("sagex_ui").join("vault")
}

fn write_private(path: &Path, bytes: &[u8]) -> VaultResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| VaultError::Io(e.to_string()))?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .and_then(|mut f| f.write_all(bytes))
            .map_err(|e| VaultError::Io(e.to_string()))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, bytes).map_err(|e| VaultError::Io(e.to_string()))?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Vault {
    dir: PathBuf,
}

impl Vault {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn default() -> Self {
        Self::new(default_vault_dir())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Store the session, AES-GCM-encrypted under `password` (fresh random
    /// salt per write, so keygen and vault uses stay domain-separated even
    /// with the same login password).
    pub fn store_session(&self, password: &[u8], sess: &StoredSession) -> VaultResult<()> {
        let pw = Zeroizing::new(password.to_vec());
        let plain = serde_json::to_vec(sess).map_err(|e| VaultError::Decode(e.to_string()))?;
        let encap = AESHandler::encrypt_private_key(&plain, &pw)
            .map_err(|e| VaultError::Crypto(format!("{e:?}")))?;
        let mut buf = Vec::new();
        encap
            .write_le(&mut Cursor::new(&mut buf))
            .map_err(|e| VaultError::Format(e.to_string()))?;
        write_private(&self.path(SESSION_FILE), &buf)
    }

    /// Load the session. Wrong password (or tampered file) fails closed.
    pub fn load_session(&self, password: &[u8]) -> VaultResult<StoredSession> {
        let pw = Zeroizing::new(password.to_vec());
        let bytes =
            std::fs::read(self.path(SESSION_FILE)).map_err(|e| VaultError::Io(e.to_string()))?;
        let encap = sagex_crypto::aes::KeyEncapsulation::read_le(&mut Cursor::new(&bytes))
            .map_err(|e| VaultError::Format(e.to_string()))?;
        let plain = AESHandler::decrypt_private_key(&pw, encap)
            .map_err(|e| VaultError::Crypto(format!("{e:?}")))?;
        serde_json::from_slice(&plain).map_err(|e| VaultError::Decode(e.to_string()))
    }

    pub fn has_session(&self) -> bool {
        self.path(SESSION_FILE).is_file()
    }

    /// Store the `sagex-format` keypair (secrets inside are already
    /// password-encapsulated; files additionally get 0600).
    pub fn store_identity(
        &self,
        prv: &PrivateExternal,
        publ: &PublicFileFormatExternal,
    ) -> VaultResult<()> {
        let mut pbuf = Vec::new();
        prv.write(&mut Cursor::new(&mut pbuf))
            .map_err(|e| VaultError::Format(e.to_string()))?;
        write_private(&self.path(PRV_FILE), &pbuf)?;
        let mut ubuf = Vec::new();
        publ.write(&mut Cursor::new(&mut ubuf))
            .map_err(|e| VaultError::Format(e.to_string()))?;
        write_private(&self.path(PUB_FILE), &ubuf)
    }

    pub fn load_prv(&self) -> VaultResult<PrivateExternal> {
        let bytes =
            std::fs::read(self.path(PRV_FILE)).map_err(|e| VaultError::Io(e.to_string()))?;
        PrivateExternal::read(&mut Cursor::new(&bytes))
            .map_err(|e| VaultError::Format(e.to_string()))
    }

    pub fn load_pub(&self) -> VaultResult<PublicFileFormatExternal> {
        let bytes =
            std::fs::read(self.path(PUB_FILE)).map_err(|e| VaultError::Io(e.to_string()))?;
        PublicFileFormatExternal::read(&mut Cursor::new(&bytes))
            .map_err(|e| VaultError::Format(e.to_string()))
    }

    pub fn has_identity(&self) -> bool {
        self.path(PRV_FILE).is_file() && self.path(PUB_FILE).is_file()
    }

    /// Store the issued device certificate (public PEM + metadata).
    pub fn store_cert(&self, identity: &str, serial: u64, pem: &str) -> VaultResult<()> {
        write_private(&self.path(CERT_PEM_FILE), pem.as_bytes())?;
        let meta = CertMeta {
            identity: identity.to_string(),
            serial,
        };
        let bytes = serde_json::to_vec(&meta).map_err(|e| VaultError::Decode(e.to_string()))?;
        write_private(&self.path(CERT_META_FILE), &bytes)
    }

    /// Load the device certificate, if enrolled.
    pub fn load_cert(&self) -> VaultResult<Option<(CertMeta, String)>> {
        let meta_path = self.path(CERT_META_FILE);
        let pem_path = self.path(CERT_PEM_FILE);
        if !meta_path.is_file() || !pem_path.is_file() {
            return Ok(None);
        }
        let meta: CertMeta = serde_json::from_slice(
            &std::fs::read(meta_path).map_err(|e| VaultError::Io(e.to_string()))?,
        )
        .map_err(|e| VaultError::Decode(e.to_string()))?;
        let pem =
            String::from_utf8(std::fs::read(pem_path).map_err(|e| VaultError::Io(e.to_string()))?)
                .map_err(|e| VaultError::Decode(e.to_string()))?;
        Ok(Some((meta, pem)))
    }

    /// Drop everything (sign-out / reset). Best effort per file.
    pub fn clear(&self) {
        for name in [
            SESSION_FILE,
            PRV_FILE,
            PUB_FILE,
            CERT_PEM_FILE,
            CERT_META_FILE,
        ] {
            let _ = std::fs::remove_file(self.path(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::session::now_unix;

    fn tmp_vault(tag: &str) -> Vault {
        let dir = std::env::temp_dir().join(format!(
            "sagex_ui_vault_{tag}_{}_{}",
            std::process::id(),
            now_unix()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Vault::new(dir)
    }

    fn sample_session() -> StoredSession {
        StoredSession {
            access_token: "a".into(),
            access_expires_at: now_unix() + 900,
            refresh_token: "r".into(),
            refresh_expires_at: now_unix() + 2_592_000,
            user_id: "u".into(),
            email: "e@x.com".into(),
            saved_at: now_unix(),
        }
    }

    #[test]
    fn session_roundtrip_and_wrong_password() {
        let v = tmp_vault("sess");
        assert!(!v.has_session());
        v.store_session(b"correct horse", &sample_session())
            .unwrap();
        assert!(v.has_session());
        let back = v.load_session(b"correct horse").unwrap();
        assert_eq!(back.email, "e@x.com");
        assert_eq!(back.refresh_token, "r");
        // Wrong password fails closed (AES-GCM auth).
        assert!(v.load_session(b"wrong password").is_err());
        // File is not plaintext JSON.
        let raw = std::fs::read(v.path(SESSION_FILE)).unwrap();
        assert!(!raw.windows(7).any(|w| w == b"e@x.com"));
        v.clear();
        assert!(!v.has_session());
    }

    #[test]
    fn cert_roundtrip() {
        let v = tmp_vault("cert");
        assert!(v.load_cert().unwrap().is_none());
        v.store_cert("alice", 42, "-----BEGIN CERTIFICATE-----\nabc\n")
            .unwrap();
        let (meta, pem) = v.load_cert().unwrap().unwrap();
        assert_eq!(meta.identity, "alice");
        assert_eq!(meta.serial, 42);
        assert!(pem.contains("BEGIN CERTIFICATE"));
        v.clear();
    }
}
