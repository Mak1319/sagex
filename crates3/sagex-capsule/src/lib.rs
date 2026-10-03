// use std::collections::HashMap;

use sagex_crypto::MlKem768;
use sagex_crypto::aes::{DekSealer, KEY_LEN, KemOps, NONCE_LEN};
use sagex_keys::PublicKey;
use serde::{Deserialize, Serialize};

use crate::error::{
    CapResult,
    Error::{DekWrapError, InvalidRecipientKey},
};

pub mod error;
pub mod format;
pub mod verify;

// /// This is the full encrypted file with the buffer and one packet per recipient.
// #[derive(Serialize, Deserialize)]
// pub struct Capsule {
//     checksum: Option<u32>,
//     ecc: Option<Vec<u8>>,
//     buffer: Vec<u8>,
// }
//
// pub struct SuperCapsule {
//     buffer: Vec<u8>, // holds capsule buffer after serialization
//     signature: Option<Vec<u8>>,
// }

/// This is the packet which is stored for one recipient.
///
/// What it does it holds the wrapped DEK and the KEM cipher text and
/// the nonce so the recipient can get the DEK back.
#[derive(Serialize, Deserialize)]
pub struct DekEncapsulation {
    nonce: [u8; NONCE_LEN],
    dek_ciphertext: Vec<u8>,
    kem_ciphertext: Vec<u8>,
}

impl DekEncapsulation {
    /// What it does it wraps the given DEK for the recipient public key and makes a fresh nonce for it.
    pub fn new(aes_key: [u8; KEY_LEN], public_key: PublicKey) -> CapResult<Self> {
        let (kem_ciphertext, secret_key) =
            <MlKem768 as KemOps>::encapsulate_bytes(&public_key.kem_key)
                .map_err(|_| InvalidRecipientKey)?;

        let (nonce, dek_ciphertext) = DekSealer::new(secret_key)
            .seal(&aes_key)
            .map_err(|_| DekWrapError)?;

        Ok(Self {
            nonce,
            dek_ciphertext,
            kem_ciphertext,
        })
    }

    /// This is the maker from wire parts when reading an archive back.
    ///
    /// What it does it holds already wrapped bytes without touching any key.
    pub fn from_parts(
        nonce: [u8; NONCE_LEN],
        dek_ciphertext: Vec<u8>,
        kem_ciphertext: Vec<u8>,
    ) -> Self {
        Self {
            nonce,
            dek_ciphertext,
            kem_ciphertext,
        }
    }

    /// This is the getter for the wrap nonce kept inside.
    ///
    /// What it does it gives back the nonce this packet was sealed with.
    pub fn nonce(&self) -> &[u8; NONCE_LEN] {
        &self.nonce
    }

    /// This is the getter for the wrapped DEK bytes kept inside.
    ///
    /// What it does it gives back the locked DEK bytes.
    pub fn dek_ciphertext(&self) -> &[u8] {
        &self.dek_ciphertext
    }

    /// This is the getter for the KEM packet bytes kept inside.
    ///
    /// What it does it gives back the key exchange bytes.
    pub fn kem_ciphertext(&self) -> &[u8] {
        &self.kem_ciphertext
    }
}

pub mod options {
    use crate::{
        DekEncapsulation,
        error::{CapResult, Error::KeyDecryptionError},
    };
    use sagex_crypto::MlKem768;
    use sagex_crypto::aes::{DekSealer, KemOps, KeyDerivedEncapsulation, generate_key};
    use sagex_keys::InternalKey;

    use super::error::Error::{DecapsulationError, DekUnwrapError};

    /// This is the helper which holds the kem key after it is derived from the password.
    pub struct DekDecapsulationOptions {
        kem_key: KeyDerivedEncapsulation,
    }

    impl DekDecapsulationOptions {
        pub fn derive_kem_key(
            password: &[u8],
            key_internal: InternalKey,
        ) -> CapResult<Self> {
            let (salt, round) = key_internal.get_kem_deliverable();
            let aes_key = generate_key(password, salt, round);

            let kem_key = key_internal
                .get_kem_key(aes_key)
                .map_err(|_| KeyDecryptionError)?;

            Ok(Self { kem_key })
        }

        /// What it does it takes the stored encapsulation and gives back the DEK using the already derived kem key.
        pub fn generate_dek(&self, enc: &DekEncapsulation) -> CapResult<[u8; 32]> {
            let secret_key = <MlKem768 as KemOps>::decapsulate_bytes(
                self.kem_key.secret_bytes(),
                &enc.kem_ciphertext,
            )
            .map_err(|_| DecapsulationError)?;
            DekSealer::new(secret_key)
                .open(&enc.nonce, &enc.dek_ciphertext)
                .map_err(|_| DekUnwrapError)
        }
    }
}
