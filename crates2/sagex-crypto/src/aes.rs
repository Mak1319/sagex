use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use rand::{TryRng, rngs::SysRng};
use sha2::Sha256;

use crate::error::{
    SageXCryptoError::{self, AESDecryptionError, RngError},
    SageXResult,
};

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const ITERATIONS: u32 = 600_000;

pub struct AESHandler;

/// Password-wrapped private key envelope.
///
/// Manual canonical byte layout (no codec crate):
/// `salt[16] || nonce[12] || cipher_len:u32 LE || cipher`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeyEncapsulation {
    pub salt: [u8; SALT_LEN],
    pub nonce_bytes: [u8; NONCE_LEN],
    pub cipher: Vec<u8>,
}

impl KeyEncapsulation {
    pub fn to_bytes(&self) -> SageXResult<Vec<u8>> {
        let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + 4 + self.cipher.len());
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&self.nonce_bytes);
        out.extend_from_slice(&(self.cipher.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.cipher);
        Ok(out)
    }

    pub fn from_bytes(bytes: &[u8]) -> SageXResult<Self> {
        use SageXCryptoError::CodecError;
        if bytes.len() < SALT_LEN + NONCE_LEN + 4 {
            return Err(CodecError);
        }
        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        salt.copy_from_slice(&bytes[..SALT_LEN]);
        nonce_bytes.copy_from_slice(&bytes[SALT_LEN..SALT_LEN + NONCE_LEN]);
        let len = u32::from_le_bytes(
            bytes[SALT_LEN + NONCE_LEN..SALT_LEN + NONCE_LEN + 4]
                .try_into()
                .map_err(|_| CodecError)?,
        ) as usize;
        let body = &bytes[SALT_LEN + NONCE_LEN + 4..];
        if body.len() != len {
            return Err(CodecError);
        }
        Ok(Self {
            salt,
            nonce_bytes,
            cipher: body.to_vec(),
        })
    }
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

        SysRng.try_fill_bytes(&mut salt).map_err(|_| RngError)?;
        SysRng
            .try_fill_bytes(&mut nonce_bytes)
            .map_err(|_| RngError)?;

        let mut aes_key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(password, &salt, ITERATIONS, &mut aes_key);

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
        encapsulation: &KeyEncapsulation,
    ) -> SageXResult<Vec<u8>> {
        let mut aes_key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(password, &encapsulation.salt, ITERATIONS, &mut aes_key);

        let cipher =
            Aes256Gcm::new_from_slice(&aes_key).map_err(|_| SageXCryptoError::AESCreationError)?;

        let nonce = Nonce::from(encapsulation.nonce_bytes);
        let plain_key = cipher
            .decrypt(&nonce, encapsulation.cipher.as_slice())
            .map_err(|_| AESDecryptionError)?;

        Ok(plain_key.to_vec())
    }
}

