use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use rand_core::{OsRng, RngCore};
use sha2::Sha256;

use crate::error::{
    SageXCryptoError::{self, AESDecryptionError},
    SageXResult,
};

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const ITERATIONS: u32 = 600_000;

pub struct AESHandler;

#[binrw::binrw]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyEncapsulation {
    pub salt: [u8; SALT_LEN],
    pub nonce_bytes: [u8; NONCE_LEN],
    #[bw(calc = cipher.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    len: u32,

    #[br(count = len )]
    pub cipher: Vec<u8>,
}

impl AESHandler {
    /// Raw AES-256-GCM encrypt with an explicit key/nonce (no KDF).
    /// For message/file DEKs; password-based flows keep using
    /// [`AESHandler::encrypt_private_key`].
    pub fn encrypt_raw(
        key: &[u8; 32],
        nonce_bytes: &[u8; NONCE_LEN],
        plaintext: &[u8],
    ) -> SageXResult<Vec<u8>> {
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(key)
            .map_err(|_| SageXCryptoError::AESCreationError)?;
        let nonce = Nonce::from(*nonce_bytes);
        cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| SageXCryptoError::AESEncryptionError)
    }

    /// Raw AES-256-GCM decrypt with an explicit key/nonce.
    pub fn decrypt_raw(
        key: &[u8; 32],
        nonce_bytes: &[u8; NONCE_LEN],
        ciphertext: &[u8],
    ) -> SageXResult<Vec<u8>> {
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(key)
            .map_err(|_| SageXCryptoError::AESCreationError)?;
        let nonce = Nonce::from(*nonce_bytes);
        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| AESDecryptionError)
    }

    pub fn encrypt_private_key(key: &[u8], password: &[u8]) -> SageXResult<KeyEncapsulation> {
        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];

        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce_bytes);

        let mut aes_key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(password, &salt, ITERATIONS, &mut aes_key);
        // println!("{:?}", &aes_key);

        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&aes_key).map_err(|e| {
            println!("{:?}", e);
            SageXCryptoError::AESCreationError
        })?;

        let nonce = Nonce::from(nonce_bytes);

        let ciphertext = cipher
            .encrypt(&nonce, key)
            .map_err(|_| SageXCryptoError::AESEncryptionError)?;

        aes_key.fill(0);
        Ok(KeyEncapsulation {
            salt,
            nonce_bytes,
            cipher: ciphertext,
        })
    }

    pub fn decrypt_private_key(
        password: &[u8],
        encapsulation: KeyEncapsulation,
    ) -> SageXResult<Vec<u8>> {
        let mut aes_key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(password, &encapsulation.salt, ITERATIONS, &mut aes_key);

        let cipher =
            Aes256Gcm::new_from_slice(&aes_key).map_err(|_| SageXCryptoError::AESCreationError)?;

        let nonce = Nonce::from(encapsulation.nonce_bytes.clone());
        let plain_key = cipher
            .decrypt(&nonce, encapsulation.cipher.as_slice())
            .map_err(|_| AESDecryptionError)?;

        Ok(plain_key.to_vec())
    }
}
