//! CA key material: load-or-generate `.prv`/`.pub` in `sagex-format`,
//! mirroring `sagex_test`, plus identity validation.

use binrw::{BinRead, BinWrite};
use sagex_crypto::{
    aes::{KeyEncapsulation, ITERATIONS},
    pqc::{dsa, kem},
};
use sagex_format::format::{
    KeyDerivatinMechanism::Password, MAGIC_NUMBER, PrivateExternal, PrivateInternal,
    PublicFileFormatExternal, PublicInternal, VERSION,
};
use std::{
    fmt,
    fs::File,
    io::Cursor,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

use crate::mldsa::MlDsa65Signer;

/// File names inside `CA_KEY_DIR`. Same names `sagex_test` writes so the
/// isolated test-env story stays familiar.
pub const PRV_FILE: &str = ".prv";
pub const PUB_FILE: &str = ".pub";

#[derive(Debug)]
pub enum CaError {
    BadIdentity(String),
    Io(std::io::Error),
    Format(binrw::Error),
    Crypto(String),
    MissingPrv(PathBuf),
}

impl fmt::Display for CaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadIdentity(s) => write!(f, "bad CA identity: {s}"),
            Self::Io(e) => write!(f, "CA key I/O error: {e}"),
            Self::Format(e) => write!(f, "CA key format error: {e}"),
            Self::Crypto(e) => write!(f, "CA crypto error: {e}"),
            Self::MissingPrv(p) => write!(
                f,
                "CA private key not found at {} (start the server once to generate it)",
                p.display()
            ),
        }
    }
}

impl std::error::Error for CaError {}

/// Identity rules shared by the CA name and CSR identities: CN-safe,
/// bounded, no whitespace or X.500 metacharacters.
pub fn valid_identity(s: &str) -> bool {
    const MAX: usize = 64;
    !s.is_empty()
        && s.len() <= MAX
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// Loaded CA material. The DSA secret stays AES-encapsulated (serialized
/// bytes) until [`CaMaterial::signer`] decrypts it; the password itself is
/// never stored.
#[derive(Clone)]
pub struct CaMaterial {
    pub identity: String,
    pub dsa_public: Vec<u8>,
    pub kem_public: Vec<u8>,
    dsa_encap_bytes: Vec<u8>,
}

impl CaMaterial {
    fn prv_path(dir: &Path) -> PathBuf {
        dir.join(PRV_FILE)
    }
    fn pub_path(dir: &Path) -> PathBuf {
        dir.join(PUB_FILE)
    }

    fn read_prv(dir: &Path) -> Result<PrivateExternal, CaError> {
        let path = Self::prv_path(dir);
        if !path.exists() {
            return Err(CaError::MissingPrv(path));
        }
        let bytes = std::fs::read(&path).map_err(CaError::Io)?;
        PrivateExternal::read(&mut Cursor::new(&bytes)).map_err(CaError::Format)
    }

    fn read_pub(dir: &Path) -> Result<PublicFileFormatExternal, CaError> {
        let bytes = std::fs::read(Self::pub_path(dir)).map_err(CaError::Io)?;
        PublicFileFormatExternal::read(&mut Cursor::new(&bytes)).map_err(CaError::Format)
    }

    fn from_pair(
        prv: PrivateExternal,
        publ: PublicFileFormatExternal,
    ) -> Result<Self, CaError> {
        if prv.internal.user_name != publ.internal.user_name {
            return Err(CaError::Format(binrw::Error::AssertFail {
                pos: 0,
                message: "CA .prv/.pub identity mismatch".to_string(),
            }));
        }
        let mut encap_bytes = Vec::new();
        prv.internal
            .key_encapsulation_dsa
            .write_le(&mut Cursor::new(&mut encap_bytes))
            .map_err(CaError::Format)?;
        Ok(Self {
            identity: prv.internal.user_name,
            dsa_public: publ.internal.key_dsa,
            kem_public: publ.internal.key_kem,
            dsa_encap_bytes: encap_bytes,
        })
    }

    /// Load existing keys; fail (no silent generation) for offline tooling.
    pub fn load(dir: &Path) -> Result<Self, CaError> {
        Self::from_pair(Self::read_prv(dir)?, Self::read_pub(dir)?)
    }

    /// Load, or generate once via the `sagex_test` flow and persist.
    /// Used by the server at startup; the CLI uses [`CaMaterial::load`].
    /// Returns the material plus whether keys were freshly generated.
    pub fn load_or_generate(
        dir: &Path,
        password: &[u8],
        identity: &str,
    ) -> Result<(Self, bool), CaError> {
        if !valid_identity(identity) {
            return Err(CaError::BadIdentity(identity.to_string()));
        }
        if Self::prv_path(dir).exists() {
            let ca = Self::load(dir)?;
            if ca.identity != identity {
                return Err(CaError::BadIdentity(format!(
                    "existing CA is {:?}, but CA_IDENTITY is {:?}",
                    ca.identity, identity
                )));
            }
            return Ok((ca, false));
        }
        std::fs::create_dir_all(dir).map_err(CaError::Io)?;

        let (kem_enc, kem_pk) = kem::KeyGen::generate_from_password(password)
            .map_err(|e| CaError::Crypto(format!("{e:?}")))?;
        let (dsa_enc, dsa_pk) = dsa::KeyGen::generate_from_password(password)
            .map_err(|e| CaError::Crypto(format!("{e:?}")))?;

        let prv = PrivateExternal {
            internal: PrivateInternal {
                magic_number: MAGIC_NUMBER,
                version: VERSION,
                key_encapsulation_kem: kem_enc,
                key_encapsulation_dsa: dsa_enc,
                user_name: identity.to_string(),
                key_derivation_mechanism_kem: Password,
                key_derivation_mechanism_dsa: Password,
                iteration_count: ITERATIONS as usize,
            },
            identity: identity.as_bytes().to_vec(),
        };
        let mut prv_file = File::create(Self::prv_path(dir)).map_err(CaError::Io)?;
        prv.write(&mut prv_file).map_err(CaError::Format)?;

        let publ = PublicFileFormatExternal {
            internal: PublicInternal {
                magic_number: MAGIC_NUMBER,
                version: VERSION,
                key_kem: kem_pk,
                key_dsa: dsa_pk,
                user_name: identity.to_string(),
            },
            ecc: vec![],
            identity: identity.as_bytes().to_vec(),
        };
        let mut pub_file = File::create(Self::pub_path(dir)).map_err(CaError::Io)?;
        publ.write(&mut pub_file).map_err(CaError::Format)?;

        Ok((Self::load(dir)?, true))
    }

    /// Decrypt the CA DSA secret and build a signer. The caller's password
    /// copy is zeroized; the secret lives in [`MlDsa65Signer`] (zeroized on drop).
    pub fn signer(&self, password: &[u8]) -> Result<MlDsa65Signer, CaError> {
        let pw = Zeroizing::new(password.to_vec());
        let encap = KeyEncapsulation::read_le(&mut Cursor::new(&self.dsa_encap_bytes))
            .map_err(CaError::Format)?;
        let sk_bytes =
            sagex_crypto::aes::AESHandler::decrypt_private_key(&pw, encap)
                .map_err(|e| CaError::Crypto(format!("{e:?}")))?;
        MlDsa65Signer::new(sk_bytes, self.dsa_public.clone())
            .map_err(|e| CaError::Crypto(format!("{e:?}")))
    }
}
