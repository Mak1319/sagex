use aes_gcm::{Aes256Gcm, Nonce, aead::Aead};
use ml_dsa::KeyInit;
use rand::{RngExt, rand_core::UnwrapErr, rngs::SysRng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::ZeroizeOnDrop;

use crate::{
    aes::key_algorithm::KeyAlgorithm,
    error::{
        SResult,
        SagexCrypotError::{
            AESDecryptionError, AESEncryptionError, AESKeyDerivationError, NonceDerivationError,
        },
    },
};

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;
pub const HASH_RND: u32 = 600_000;

pub const KEM_EK_LEN: usize = 1184;
pub const KEM_DK_SEED_LEN: usize = 64;
pub const KEM_CT_LEN: usize = 1088;
/// ML-DSA-44 verifying key length.
pub const DSA44_VK_LEN: usize = 1312;

pub use dsa::{dsa44_sign, dsa44_verify_feed};
pub use kem::KemOps;

/// This is the encapsulation which should be used for encapsulating
/// post quantum cryptography keys
///
/// What it does it encrypts the PQC keys and convert them into cipher text
/// and it can be decrypted by using password  
#[derive(Serialize, Deserialize)]
pub struct KeyEncapsulation {
    secret_key: Vec<u8>,
    pub salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
    pub public_key: Vec<u8>,
    pub rounds: u32,
}

/// This is intermediate and should not be serialize and deserialize
#[derive(ZeroizeOnDrop)]
pub struct KeyDerivedEncapsulation {
    secret_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl KeyDerivedEncapsulation {
    /// What it does it gives back the raw private key bytes which are kept inside.
    pub fn secret_bytes(&self) -> &[u8] {
        &self.secret_key
    }
}

/// This is the box which holds the shared secret and it seals and opens
/// the DEK with it.
///
/// What it does it takes the secret once and then you can wrap the DEK
/// and get it back later. It wipes the secret when it is dropped so
/// nothing stays in memory.
#[derive(ZeroizeOnDrop)]
pub struct DekSealer {
    secret_key: [u8; KEY_LEN],
}

impl DekSealer {
    pub fn new(secret_key: [u8; KEY_LEN]) -> Self {
        Self { secret_key }
    }

    /// What it does it wraps the given DEK with the secret and makes a fresh random nonce for it.
    pub fn seal(&self, dek: &[u8; KEY_LEN]) -> SResult<([u8; NONCE_LEN], Vec<u8>)> {
        let mut rng = UnwrapErr(SysRng);
        let nonce_bytes: [u8; NONCE_LEN] = rng.random();
        let cipher =
            Aes256Gcm::new_from_slice(&self.secret_key).map_err(|_| AESKeyDerivationError)?;
        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| NonceDerivationError)?;
        let ciphertext = cipher
            .encrypt(&nonce, dek.as_ref())
            .map_err(|_| AESEncryptionError)?;
        Ok((nonce_bytes, ciphertext))
    }

    /// What it does it takes the wrapped DEK back using the secret and the nonce.
    pub fn open(&self, nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> SResult<[u8; KEY_LEN]> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.secret_key).map_err(|_| AESKeyDerivationError)?;
        let nonce = Nonce::try_from(*nonce).map_err(|_| NonceDerivationError)?;
        let pt = cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| AESDecryptionError)?;
        pt.as_slice().try_into().map_err(|_| AESDecryptionError)
    }

    /// What it does it locks any length bytes with the secret and makes a fresh random nonce for them.
    pub fn seal_bytes(&self, plaintext: &[u8]) -> SResult<([u8; NONCE_LEN], Vec<u8>)> {
        let mut rng = UnwrapErr(SysRng);
        let nonce_bytes: [u8; NONCE_LEN] = rng.random();
        let ciphertext = self.seal_bytes_with(&nonce_bytes, plaintext)?;
        Ok((nonce_bytes, ciphertext))
    }

    /// What it does it locks any length bytes with the secret and the nonce you give.
    pub fn seal_bytes_with(
        &self,
        nonce_bytes: &[u8; NONCE_LEN],
        plaintext: &[u8],
    ) -> SResult<Vec<u8>> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.secret_key).map_err(|_| AESKeyDerivationError)?;
        let nonce = Nonce::try_from(*nonce_bytes).map_err(|_| NonceDerivationError)?;
        cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| AESEncryptionError)
    }

    /// What it does it takes back any length bytes locked with the secret and the nonce.
    pub fn open_bytes(&self, nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> SResult<Vec<u8>> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.secret_key).map_err(|_| AESKeyDerivationError)?;
        let nonce = Nonce::try_from(*nonce).map_err(|_| NonceDerivationError)?;
        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| AESDecryptionError)
    }
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

    pub fn decrypt(&self, password: &[u8]) -> SResult<KeyDerivedEncapsulation> {
        let mut key_bytes = [0u8; KEY_LEN];
        pbkdf2::pbkdf2_hmac::<Sha256>(password, &self.salt, self.rounds, &mut key_bytes);

        let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| AESKeyDerivationError)?;
        let nonce = Nonce::try_from(self.nonce).map_err(|_| NonceDerivationError)?;

        let derivable_text = cipher
            .decrypt(&nonce, self.secret_key.as_slice())
            .map_err(|_| AESDecryptionError)?;

        Ok(KeyDerivedEncapsulation {
            secret_key: derivable_text,
            public_key: self.public_key.clone(),
        })
    }

    pub fn decrypt_from_password(
        &self,
        aes_key: [u8; KEY_LEN],
    ) -> SResult<KeyDerivedEncapsulation> {
        let cipher = Aes256Gcm::new_from_slice(&aes_key).map_err(|_| AESKeyDerivationError)?;
        let nonce = Nonce::try_from(self.nonce).map_err(|_| NonceDerivationError)?;

        let derivable_text = cipher
            .decrypt(&nonce, self.secret_key.as_slice())
            .map_err(|_| AESDecryptionError)?;

        Ok(KeyDerivedEncapsulation {
            secret_key: derivable_text,
            public_key: self.public_key.clone(),
        })
    }

    pub fn get_password_deliverables(&self) -> ([u8; SALT_LEN], u32) {
        (self.salt, self.rounds)
    }
}

impl PartialEq for KeyEncapsulation {
    fn eq(&self, other: &Self) -> bool {
        self.salt == other.salt && self.rounds == other.rounds
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

pub mod key_algorithm {
    use crate::error::SResult;

    pub trait KeyAlgorithm {
        type PublicKey;
        type PrivateKey;
        fn generate() -> (Self::PublicKey, Self::PrivateKey);

        fn public_key_bytes(key: &Self::PublicKey) -> Vec<u8>;
        fn private_key_bytes(key: &Self::PrivateKey) -> Vec<u8>;
    }

    /// This is the trait which should be used for KEM with real key types.
    ///
    /// What it does it encapsulates a new shared secret for the public key
    /// and decapsulates it back with the private key. It is implemented
    /// for MlKem768 and MlDsa65 should not have it because it is not a KEM.
    pub trait Kemable: KeyAlgorithm {
        type Ciphertext;
        type SharedSecret;

        fn encapsulate(ek: &Self::PublicKey) -> SResult<(Self::Ciphertext, Self::SharedSecret)>;
        fn decapsulate(dk: &Self::PrivateKey, ct: &Self::Ciphertext)
        -> SResult<Self::SharedSecret>;
    }
}
mod dsa {
    use ml_dsa::{Generate, KeyExport, Keypair, MlDsa44, MlDsa65, SigningKey, VerifyingKey};

    use crate::aes::key_algorithm::KeyAlgorithm;
    use crate::error::SResult;

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

    impl KeyAlgorithm for MlDsa44 {
        type PrivateKey = SigningKey<MlDsa44>;
        type PublicKey = VerifyingKey<MlDsa44>;

        fn generate() -> (Self::PublicKey, Self::PrivateKey) {
            let private_key = SigningKey::<MlDsa44>::generate();
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

    /// Sign a message with an ML-DSA-44 seed, deterministically.
    ///
    /// What it does it rebuilds the signing key from its seed and signs
    /// the whole message at once. Create-time data is already resident,
    /// so no streaming is needed on this path; streaming lives only on
    /// the verify side.
    pub fn dsa44_sign(signing_seed: &[u8], message_bytes: &[u8]) -> SResult<Vec<u8>> {
        use crate::error::SagexCrypotError::SignatureSignError;
        use digest::Update;
        use ml_dsa::signature::DigestSigner;
        use ml_dsa::{KeyInit, MlDsa44, SigningKey};

        if signing_seed.len() != 32 {
            return Err(SignatureSignError);
        }
        let mut seed_raw = [0u8; 32];
        seed_raw.copy_from_slice(signing_seed);
        let signing_key = SigningKey::<MlDsa44>::new(&seed_raw.into());
        let produced_signature = signing_key
            .try_sign_digest(|running_hash| {
                Update::update(running_hash, message_bytes);
                Ok(())
            })
            .map_err(|_| SignatureSignError)?;
        Ok(produced_signature.encode().as_slice().to_vec())
    }

    /// Verify an ML-DSA-44 signature over streamed content.
    ///
    /// What it does it checks the signature without holding the whole
    /// message: `provide_signed_bytes` is called once and must hand every
    /// signed byte to the hash in order; only reading failures surface as
    /// [`StreamReadError`], everything else is [`SignatureVerifyError`].
    pub fn dsa44_verify_feed<F>(
        sender_public_key_bytes: &[u8],
        signature_bytes: &[u8],
        provide_signed_bytes: F,
    ) -> SResult<()>
    where
        F: FnOnce(&mut dyn FnMut(&[u8])) -> SResult<()>,
    {
        use crate::error::SagexCrypotError::{SignatureVerifyError, StreamReadError};
        use digest::Update;
        use ml_dsa::signature::Error as SigError;
        use ml_dsa::{KeyInit, MlDsa44, Signature, VerifyingKey};

        let raw_public_key: [u8; crate::aes::DSA44_VK_LEN] = sender_public_key_bytes
            .try_into()
            .map_err(|_| SignatureVerifyError)?;
        let sender_public_key = VerifyingKey::<MlDsa44>::new(&raw_public_key.into());
        let parsed_signature =
            Signature::<MlDsa44>::try_from(signature_bytes).map_err(|_| SignatureVerifyError)?;
        let computed_message_hash = sender_public_key
            .compute_mu(
                |running_hash| {
                    provide_signed_bytes(&mut |piece_bytes| {
                        Update::update(running_hash, piece_bytes)
                    })
                    .map_err(|_| SigError::new())?;
                    Ok(())
                },
                &[],
            )
            .map_err(|_| StreamReadError)?;
        if sender_public_key.verify_mu(&computed_message_hash, &parsed_signature) {
            Ok(())
        } else {
            Err(SignatureVerifyError)
        }
    }
}
mod kem {
    use ml_dsa::KeyExport;
    use ml_kem::{Ciphertext, Decapsulate, Encapsulate, Kem, MlKem768, SharedKey};

    use crate::{
        aes::key_algorithm::{Kemable, KeyAlgorithm},
        error::SResult,
    };

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

    impl Kemable for MlKem768 {
        type Ciphertext = Ciphertext<MlKem768>;
        type SharedSecret = SharedKey;

        fn encapsulate(ek: &Self::PublicKey) -> SResult<(Self::Ciphertext, Self::SharedSecret)> {
            Ok(Encapsulate::encapsulate(ek))
        }

        fn decapsulate(
            dk: &Self::PrivateKey,
            ct: &Self::Ciphertext,
        ) -> SResult<Self::SharedSecret> {
            Ok(Decapsulate::decapsulate(dk, ct))
        }
    }

    /// This is the trait which should be used for KEM work with raw bytes.
    ///
    /// What it does it encapsulates and decapsulates without touching
    /// ml-kem types so other crates can use it. It is implemented
    /// for MlKem768.
    pub trait KemOps {
        fn encapsulate_bytes(ek_bytes: &[u8]) -> SResult<(Vec<u8>, [u8; crate::aes::KEY_LEN])>;
        fn decapsulate_bytes(dk_seed: &[u8], ct_bytes: &[u8])
        -> SResult<[u8; crate::aes::KEY_LEN]>;
    }

    impl KemOps for MlKem768 {
        fn encapsulate_bytes(ek_bytes: &[u8]) -> SResult<(Vec<u8>, [u8; crate::aes::KEY_LEN])> {
            use crate::error::SagexCrypotError::KEMEncapsulationError;
            use ml_kem::{EncapsulationKey, MlKem768};

            let raw: [u8; crate::aes::KEM_EK_LEN] =
                ek_bytes.try_into().map_err(|_| KEMEncapsulationError)?;
            let ek = EncapsulationKey::<MlKem768>::new(&raw.into())
                .map_err(|_| KEMEncapsulationError)?;
            let (ct, ss) =
                <MlKem768 as Kemable>::encapsulate(&ek).map_err(|_| KEMEncapsulationError)?;
            let secret_key: [u8; crate::aes::KEY_LEN] = ss
                .as_slice()
                .try_into()
                .map_err(|_| KEMEncapsulationError)?;
            Ok((ct.as_slice().to_vec(), secret_key))
        }

        fn decapsulate_bytes(
            dk_seed: &[u8],
            ct_bytes: &[u8],
        ) -> SResult<[u8; crate::aes::KEY_LEN]> {
            use crate::error::SagexCrypotError::KEMDecapsulationError;
            use ml_kem::{Ciphertext, DecapsulationKey, MlKem768};

            let seed: [u8; crate::aes::KEM_DK_SEED_LEN] =
                dk_seed.try_into().map_err(|_| KEMDecapsulationError)?;
            let ct_raw: [u8; crate::aes::KEM_CT_LEN] =
                ct_bytes.try_into().map_err(|_| KEMDecapsulationError)?;
            let dk = DecapsulationKey::<MlKem768>::from_seed(seed.into());
            let ciphertext: Ciphertext<MlKem768> = ct_raw.into();
            let ss = <MlKem768 as Kemable>::decapsulate(&dk, &ciphertext)
                .map_err(|_| KEMDecapsulationError)?;
            ss.as_slice().try_into().map_err(|_| KEMDecapsulationError)
        }
    }
}

pub fn generate_key(password: &[u8], salt: [u8; SALT_LEN], rounds: u32) -> [u8; KEY_LEN] {
    let mut key_bytes = [0u8; KEY_LEN];
    pbkdf2::pbkdf2_hmac::<Sha256>(password, &salt, rounds, &mut key_bytes);
    key_bytes
}
