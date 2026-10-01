use aes_gcm::{Aes256Gcm, Nonce, aead::Aead};
use ml_dsa::KeyInit;
use rand::{RngExt, rand_core::UnwrapErr, rngs::SysRng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{
    aes::key_algorithm::KeyAlgorithm,
    error::{
        SResult,
        SagexCrypotError::{AESEncryptionError, AESKeyDerivationError, NonceDerivationError},
    },
};

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;
pub const HASH_RND: u32 = 600_000;

#[derive(Serialize, Deserialize)]
pub struct KeyEncapsulation {
    secret_key: Vec<u8>,
    salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
    pub public_key: Vec<u8>,
    rounds: u32,
}

impl KeyEncapsulation {
    pub fn new<T>(password: &[u8]) -> SResult<Self>
    where
        T: KeyAlgorithm,
    {
        let mut rng = UnwrapErr(SysRng);
        let salt_bytes: [u8; SALT_LEN] = rng.random();
        let nonce_bytes: [u8; NONCE_LEN] = rng.random();

        let mut key_bytes = [0u8; KEY_LEN];
        pbkdf2::pbkdf2_hmac::<Sha256>(&password, &salt_bytes, HASH_RND, &mut key_bytes);

        let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| AESKeyDerivationError)?;
        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| NonceDerivationError)?;

        let (public_key, private_key) = T::generate();
        let ciphertext = cipher
            .encrypt(&nonce, T::private_key_bytes(&private_key).as_ref())
            .map_err(|_| AESEncryptionError)?;

        Ok(Self {
            nonce: nonce_bytes,
            public_key: T::public_key_bytes(&public_key),
            salt: salt_bytes,
            secret_key: ciphertext,
            rounds: HASH_RND,
        })
        // private_key
    }

    pub fn from_password<T>(
        aes_key: [u8; KEY_LEN],
        salt: [u8; SALT_LEN],
        rounds: u32,
    ) -> SResult<Self>
    where
        T: KeyAlgorithm,
    {
        let mut rng = UnwrapErr(SysRng);
        let nonce_bytes: [u8; NONCE_LEN] = rng.random();

        let cipher = Aes256Gcm::new_from_slice(&aes_key).map_err(|_| AESKeyDerivationError)?;
        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| NonceDerivationError)?;

        let (public_key, private_key) = T::generate();
        let ciphertext = cipher
            .encrypt(&nonce, T::private_key_bytes(&private_key).as_ref())
            .map_err(|_| AESEncryptionError)?;

        Ok(Self {
            nonce: nonce_bytes,
            public_key: T::public_key_bytes(&public_key),
            salt: salt,
            secret_key: ciphertext,
            rounds,
        })
    }
}

mod key_algorithm {

    pub trait KeyAlgorithm {
        type PublicKey;
        type PrivateKey;
        fn generate() -> (Self::PublicKey, Self::PrivateKey);

        fn public_key_bytes(key: &Self::PublicKey) -> Vec<u8>;
        fn private_key_bytes(key: &Self::PrivateKey) -> Vec<u8>;
    }
}
mod dsa {
    use ml_dsa::{Generate, KeyExport, Keypair, MlDsa65, SigningKey, VerifyingKey};

    use crate::aes::key_algorithm::KeyAlgorithm;

    impl KeyAlgorithm for MlDsa65 {
        type PrivateKey = SigningKey<MlDsa65>;
        type PublicKey = VerifyingKey<MlDsa65>;

        fn generate() -> (Self::PublicKey, Self::PrivateKey) {
            let private_key = SigningKey::<MlDsa65>::generate();
            let verifying_key = private_key.verifying_key();
            (verifying_key, private_key)
        }
        fn private_key_bytes(key: &Self::PrivateKey) -> Vec<u8> {
            key.to_bytes().to_vec()
        }
        fn public_key_bytes(key: &Self::PublicKey) -> Vec<u8> {
            key.to_bytes().to_vec()
        }
    }
}
mod kem {
    use ml_dsa::KeyExport;
    use ml_kem::{Kem, MlKem768};

    use crate::aes::key_algorithm::KeyAlgorithm;

    impl KeyAlgorithm for MlKem768 {
        type PublicKey = <MlKem768 as Kem>::EncapsulationKey;
        type PrivateKey = <MlKem768 as Kem>::DecapsulationKey;

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
}
