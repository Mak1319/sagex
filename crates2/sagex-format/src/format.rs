use sagex_crypto::aes::KeyEncapsulation;

use crate::ecc::{EccChunk, calculate_ecc};

pub const MAGIC_NUMBER: u32 = 0x00106E5A; // This will be written as 5A6E10 in short SAGE X
pub const VERSION: u32 = 1;

/// Key-derivation mechanism for one wrapped private key.
/// Serializes as 0 (TPM) / 1 (Password) in the canonical byte layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyDerivationMechanism {
    TPM,
    Password,
}

/// Internal structure of a key.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrivateInternal {
    pub magic_number: u32,
    pub version: u32,
    pub key_encapsulation_kem: KeyEncapsulation,
    pub key_encapsulation_dsa: KeyEncapsulation,
    pub user_name: String,
    pub key_derivation_mechanism_kem: KeyDerivationMechanism,
    pub key_derivation_mechanism_dsa: KeyDerivationMechanism,
    pub iteration_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrivateExternal {
    pub internal: PrivateInternal,
    /// Reed-Solomon chunks covering [`PrivateInternal::to_canonical_bytes`].
    /// Computed at construction via [`ecc_chunks_for`], not parsed.
    pub ecc: Vec<EccChunk>,
    pub identity: Vec<u8>,
}

impl PrivateExternal {
    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicInternal {
    pub magic_number: u32,
    pub version: u32,
    pub key_kem: Vec<u8>,
    pub key_dsa: Vec<u8>,
    pub user_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicFileFormatExternal {    pub internal: PublicInternal,
    pub ecc: Vec<u8>,
    pub identity: Vec<u8>,
}

impl PublicFileFormatExternal {
    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

/// Number of Reed-Solomon ECC chunks covering `internal`. Infallible wrapper:
/// a failure degrades to zero chunks (still a symmetric file) rather than a
/// panicking writer.
pub fn ecc_chunk_count(internal: &PrivateInternal) -> u32 {
    calculate_ecc(internal).map(|v| v.len() as u32).unwrap_or(0)
}

/// ECC chunks covering `internal` (same computation as [`ecc_chunk_count`]).
pub fn ecc_chunks_for(internal: &PrivateInternal) -> Vec<EccChunk> {
    calculate_ecc(internal).unwrap_or_default()
}
