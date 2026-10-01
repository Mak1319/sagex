use crate::format::{KeyDerivationMechanism, PrivateInternal};
use reed_solomon::Encoder;

pub const ECC_LEN: usize = 32;
pub const ECC_CHUNK_SIZE: usize = 256 - ECC_LEN;

pub type EccChunk = [u8; ECC_LEN];

pub mod error {
    use std::fmt;

    #[derive(Debug)]
    pub enum EccError {
        Serialize,
        ChunkLengthMismatch {
            chunk_index: usize,
            expected: usize,
            actual: usize,
        },
    }

    impl fmt::Display for EccError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                EccError::Serialize => write!(f, "failed to serialize internal"),
                EccError::ChunkLengthMismatch {
                    chunk_index,
                    expected,
                    actual,
                } => {
                    write!(
                        f,
                        "chunk {chunk_index}: expected {expected} ecc bytes, got {actual}"
                    )
                }
            }
        }
    }

    impl std::error::Error for EccError {}
}

use error::EccError;

fn mechanism_byte(m: KeyDerivationMechanism) -> u8 {
    match m {
        KeyDerivationMechanism::TPM => 0,
        KeyDerivationMechanism::Password => 1,
    }
}

fn append_encap(buf: &mut Vec<u8>, encap: &sagex_crypto::aes::KeyEncapsulation) {
    buf.extend_from_slice(&encap.salt);
    buf.extend_from_slice(&encap.nonce_bytes);
    buf.extend_from_slice(&(encap.cipher.len() as u32).to_le_bytes());
    buf.extend_from_slice(&encap.cipher);
}

/// Deterministic canonical byte layout of [`PrivateInternal`] (all
/// integers little-endian). No codec crate involved: this is the exact
/// input the Reed-Solomon chunks are computed over.
pub fn canonical_bytes(ke: &PrivateInternal) -> Result<Vec<u8>, EccError> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&ke.magic_number.to_le_bytes());
    buf.extend_from_slice(&ke.version.to_le_bytes());
    append_encap(&mut buf, &ke.key_encapsulation_kem);
    append_encap(&mut buf, &ke.key_encapsulation_dsa);
    let user = ke.user_name.as_bytes();
    buf.extend_from_slice(&(user.len() as u32).to_le_bytes());
    buf.extend_from_slice(user);
    buf.push(mechanism_byte(ke.key_derivation_mechanism_kem));
    buf.push(mechanism_byte(ke.key_derivation_mechanism_dsa));
    let iterations = u32::try_from(ke.iteration_count).map_err(|_| EccError::Serialize)?;
    buf.extend_from_slice(&iterations.to_le_bytes());
    Ok(buf)
}

pub fn calculate_ecc(ke: &PrivateInternal) -> Result<Vec<EccChunk>, EccError> {
    let buf = canonical_bytes(ke)?;

    let encoder = Encoder::new(ECC_LEN);

    buf.chunks(ECC_CHUNK_SIZE)
        .enumerate()
        .map(|(i, chunk)| {
            let encoded_bytes = encoder.encode(chunk);
            let ecc_slice = encoded_bytes.ecc();

            ecc_slice
                .try_into()
                .map_err(|_| EccError::ChunkLengthMismatch {
                    chunk_index: i,
                    expected: ECC_LEN,
                    actual: ecc_slice.len(),
                })
        })
        .collect::<Result<Vec<EccChunk>, EccError>>()
}
