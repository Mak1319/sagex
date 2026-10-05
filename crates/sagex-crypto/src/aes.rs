use aes_gcm::KeyInit;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Nonce};
use rand::{RngExt, rand_core::UnwrapErr, rngs::SysRng};
use serde::{Deserialize, Serialize};
use zeroize::ZeroizeOnDrop;

pub use ml_dsa::{MlDsa65, SigningKey};
pub use ml_kem::{DecapsulationKey, Encapsulate, EncapsulationKey, MlKem768, TryKeyInit};

use crate::aes::key_derivation::KeyDerivation;
use crate::error::Error::{AesDecryptionError, AesEncryptionError, AesNonceDerivationError};
use crate::{
    aes::key::KeyAlgorithm,
    error::{CResult, Error::AesKeyDerivationError},
};

// This is region is for constant
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;
pub const KDF_ROUND: u32 = 600_600;

// This is for types
pub type SaltBytes = [u8; SALT_LEN];
pub type NonceBytes = [u8; NONCE_LEN];
pub type KeyBytes = [u8; KEY_LEN];
pub type BufferBytes = Vec<u8>;

// This is the portion for actual structs

#[derive(Serialize, Deserialize, Clone)]
pub struct KeyEncapsulation {
    secret_key_cipher: BufferBytes,
    salt_bytes: SaltBytes,
    nonce_bytes: NonceBytes,

    #[serde(skip)]
    pub public_key: Option<BufferBytes>,
}

#[derive(ZeroizeOnDrop)]
pub struct KeyDerived {
    secret_key: BufferBytes,
    pub public_key: Option<BufferBytes>,
}

impl KeyEncapsulation {
    pub fn new<T: KeyAlgorithm>(password: &[u8]) -> CResult<Self> {
        let mut rng = UnwrapErr(SysRng);
        let salt_bytes: SaltBytes = rng.random();
        let nonce_bytes: NonceBytes = rng.random();


        let key_bytes = KeyDerivation::derive_password(password, salt_bytes);

        let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| AesKeyDerivationError)?;

        let (public_key, private_key) = T::generate();

        let private_key_bytes = T::private_key_bytes(&private_key);
        let public_key_bytes = T::public_key_bytes(&public_key);

        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| AesNonceDerivationError)?;

        let ciphertext = cipher
            .encrypt(&nonce, private_key_bytes.as_ref())
            .map_err(|_| AesEncryptionError)?;

        Ok(Self {
            secret_key_cipher: ciphertext,
            salt_bytes,
            nonce_bytes,
            public_key: Some(public_key_bytes),
        })
    }

    pub fn from_key<T: KeyAlgorithm>(key_bytes: KeyBytes, salt_bytes: SaltBytes) -> CResult<Self> {
        let mut rng = UnwrapErr(SysRng);
        let nonce_bytes: NonceBytes = rng.random();

        let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| AesKeyDerivationError)?;
        let (public_key, private_key) = T::generate();

        let private_key_bytes = T::private_key_bytes(&private_key);
        let public_key_bytes = T::public_key_bytes(&public_key);

        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| AesNonceDerivationError)?;

        let ciphertext = cipher
            .encrypt(&nonce, private_key_bytes.as_ref())
            .map_err(|_| AesEncryptionError)?;

        Ok(Self {
            secret_key_cipher: ciphertext,
            salt_bytes,
            nonce_bytes,
            public_key: Some(public_key_bytes),
        })
    }

    pub fn decrypt_vault(&self, password: &[u8]) -> CResult<KeyDerived> {
        let key_bytes = KeyDerivation::derive_password(password, self.salt_bytes);

        let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| AesKeyDerivationError)?;

        let nonce = Nonce::try_from(self.nonce_bytes).map_err(|_| AesNonceDerivationError)?;

        let secret_key = cipher
            .decrypt(&nonce, self.secret_key_cipher.as_ref())
            .map_err(|_| AesDecryptionError)?;

        Ok(KeyDerived {
            secret_key,
            public_key: self.public_key.clone(),
        })
    }

    pub fn decrypt_vault_from_key(&self, key_bytes: KeyBytes) -> CResult<KeyDerived> {
        let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| AesKeyDerivationError)?;

        let nonce = Nonce::try_from(self.nonce_bytes).map_err(|_| AesNonceDerivationError)?;

        let secret_key = cipher
            .decrypt(&nonce, self.secret_key_cipher.as_ref())
            .map_err(|_| AesDecryptionError)?;

        Ok(KeyDerived {
            secret_key,
            public_key: self.public_key.clone(),
        })
    }
}

pub struct EncryptionBuffer;


impl EncryptionBuffer {
    pub fn encrypt(buffer: &[u8], key: KeyBytes, nonce_bytes: NonceBytes) -> CResult<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| AesKeyDerivationError)?;
        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| AesNonceDerivationError)?;
        let ciphertext = cipher
            .encrypt(&nonce, buffer)
            .map_err(|_| AesEncryptionError)?;
        Ok(ciphertext)
    }

    pub fn decrypt(buffer: &[u8], key: KeyBytes, nonce_bytes: NonceBytes) -> CResult<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| AesKeyDerivationError)?;
        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| AesNonceDerivationError)?;
        let ciphertext = cipher
            .decrypt(&nonce, buffer)
            .map_err(|_| AesEncryptionError)?;
        Ok(ciphertext)
    }
}


pub mod key_derivation {
    use rand::{RngExt, rand_core::UnwrapErr, rngs::SysRng};
    use sha2::Sha256;

    use crate::aes::{KDF_ROUND, KEY_LEN, KeyBytes, NonceBytes, SaltBytes};


    pub struct KeyDerivation;

    impl KeyDerivation {
        pub fn derive_password(password: &[u8], salt_bytes: SaltBytes) -> KeyBytes {
            let mut key_bytes: KeyBytes = [0u8; KEY_LEN];
            pbkdf2::pbkdf2_hmac::<Sha256>(password, &salt_bytes, KDF_ROUND, &mut key_bytes);
            key_bytes
        }

        pub fn get_salt() -> SaltBytes {
            let mut rng = UnwrapErr(SysRng);
            let salt: SaltBytes = rng.random();
            salt
        }

        pub fn get_nonce() -> NonceBytes {
            let mut rng = UnwrapErr(SysRng);
            let nonce: NonceBytes = rng.random();
            nonce
        }

        pub fn get_random_key() -> KeyBytes {
            let mut rng = UnwrapErr(SysRng);
            let key: KeyBytes = rng.random();
            key
        }
    }
}

pub mod key {
    use crate::{aes::KeyDerived, error::CResult};

    pub trait KeyAlgorithm {
        type PublicKey;

        type PrivateKey;

        fn generate() -> (Self::PublicKey, Self::PrivateKey);

        fn public_key_bytes(key: &Self::PublicKey) -> Vec<u8>;

        fn private_key_bytes(key: &Self::PrivateKey) -> Vec<u8>;
    }

    pub trait FromDerived {
        type PublicKey;
        type PrivateKey;

        fn private_from_derived(value: KeyDerived) -> CResult<Self::PrivateKey>;
        fn public_from_derived(value: KeyDerived) -> CResult<Self::PublicKey>;
    }
}
pub mod kem {
    pub use ml_kem::MlKem768;

    use crate::{
        aes::{
            KeyDerived,
            key::{FromDerived, KeyAlgorithm},
        },
        error::Error::{KemKeyDerivationError, KemKeyNotProvided},
    };
    use ml_kem::{
        Kem, KeyExport, KeyInit, TryKeyInit,
        kem::{DecapsulationKey, EncapsulationKey},
    };


    impl KeyAlgorithm for MlKem768 {
        type PrivateKey = <MlKem768 as Kem>::DecapsulationKey;
        type PublicKey = <MlKem768 as Kem>::EncapsulationKey;

        fn generate() -> (Self::PublicKey, Self::PrivateKey) {
            let (dk, ek) = MlKem768::generate_keypair();
            ek.to_bytes().to_vec();
            (ek, dk)
        }

        fn private_key_bytes(key: &Self::PrivateKey) -> Vec<u8> {
            key.to_bytes().to_vec()
        }
        fn public_key_bytes(key: &Self::PublicKey) -> Vec<u8> {
            key.to_bytes().to_vec()
        }
    }

    impl FromDerived for MlKem768
    where
        <MlKem768 as Kem>::DecapsulationKey: KeyInit,
        <MlKem768 as Kem>::EncapsulationKey: TryKeyInit,
    {
        type PrivateKey = <MlKem768 as Kem>::DecapsulationKey;
        type PublicKey = <MlKem768 as Kem>::EncapsulationKey;

        fn private_from_derived(value: KeyDerived) -> crate::error::CResult<Self::PrivateKey> {
            DecapsulationKey::<Self>::new_from_slice(&value.secret_key)
                .map_err(|_| KemKeyDerivationError)
        }

        fn public_from_derived(value: KeyDerived) -> crate::error::CResult<Self::PublicKey> {
            let Some(public_key) = &value.public_key else {
                return Err(KemKeyNotProvided);
            };
            EncapsulationKey::<Self>::new_from_slice(&public_key).map_err(|_| KemKeyDerivationError)
        }
    }
}
pub mod dsa {
    pub use ml_dsa::MlDsa65;

    use ml_dsa::{Generate, KeyExport, KeyInit, Keypair, SigningKey, VerifyingKey};

    use crate::{
        aes::key::{FromDerived, KeyAlgorithm},
        error::Error::DsaKeyDerivationError,
    };


    impl KeyAlgorithm for MlDsa65 {
        type PublicKey = VerifyingKey<Self>;

        type PrivateKey = SigningKey<Self>;

        fn generate() -> (Self::PublicKey, Self::PrivateKey) {
            let private_key = SigningKey::<Self>::generate();

            let verification_key = private_key.verifying_key();
            (verification_key, private_key)
        }

        fn public_key_bytes(key: &Self::PublicKey) -> Vec<u8> {
            key.to_bytes().to_vec()
        }

        fn private_key_bytes(key: &Self::PrivateKey) -> Vec<u8> {
            key.to_bytes().to_vec()
        }
    }

    impl FromDerived for MlDsa65 {
        type PublicKey = VerifyingKey<Self>;

        type PrivateKey = SigningKey<Self>;

        fn private_from_derived(
            value: super::KeyDerived,
        ) -> crate::error::CResult<Self::PrivateKey> {
            SigningKey::<MlDsa65>::new_from_slice(&value.secret_key)
                .map_err(|_| DsaKeyDerivationError)
        }

        fn public_from_derived(value: super::KeyDerived) -> crate::error::CResult<Self::PublicKey> {
            let Some(public_key) = &value.public_key else {
                return Err(crate::error::Error::DsaKeyNotProvided);
            };
            VerifyingKey::new_from_slice(&public_key).map_err(|_| DsaKeyDerivationError)
        }
    }
}
