//! Post-quantum KEM + signatures backed directly by
//! [`ml-kem`](https://docs.rs/ml-kem) (FIPS 203, ML-KEM-768) and
//! [`ml-dsa`](https://docs.rs/ml-dsa) (FIPS 204, ML-DSA-65).
//!
//! All public APIs are byte-level (`Vec<u8>` / `&[u8]`) so upstream type
//! churn never leaks into callers. Private key material is stored as a
//! fixed-size seed wrapped by [`AESHandler`](crate::aes::AESHandler).

pub mod kem {
    use ml_kem::{
        DecapsulationKey768, EncapsulationKey768, KeyExport, Seed as KemSeed, TryKeyInit,
        kem::{Decapsulate, Encapsulate},
    };
    use rand::{TryRng, rngs::SysRng};

    use crate::{
        aes::{AESHandler, KeyEncapsulation},
        error::{
            SageXCryptoError::{
                MLKEMCipherDerivationError, MLKEMPublicKeyDerivationFailed,
                MLKEMSecretKeyDerivationFailed, RngError,
            },
            SageXResult,
        },
    };

    /// ML-KEM-768 decapsulation seed length (bytes).
    pub const SEED_LEN: usize = 64;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct KeyGen;

    impl KeyGen {
        pub fn generate_from_password(password: &[u8]) -> SageXResult<(KeyEncapsulation, Vec<u8>)> {
            let mut seed = [0u8; SEED_LEN];
            SysRng.try_fill_bytes(&mut seed).map_err(|_| RngError)?;
            let seed_arr =
                KemSeed::try_from(&seed[..]).map_err(|_| MLKEMSecretKeyDerivationFailed)?;
            let dk = DecapsulationKey768::from_seed(seed_arr);
            let ek_bytes = dk.encapsulation_key().to_bytes().to_vec();
            let encap = AESHandler::encrypt_private_key(&seed, password)?;
            Ok((encap, ek_bytes))
        }
    }

    pub struct Implementation;

    impl Implementation {
        /// Byte-level encapsulation: returns `(kem_ciphertext_bytes, shared_secret_bytes)`.
        pub fn encapsulate_bytes(key: &[u8]) -> SageXResult<(Vec<u8>, Vec<u8>)> {
            let ek = EncapsulationKey768::new_from_slice(key)
                .map_err(|_| MLKEMPublicKeyDerivationFailed)?;
            let (ct, ss) = ek.encapsulate();
            Ok((ct.as_slice().to_vec(), ss.as_slice().to_vec()))
        }

        pub fn encapsulate(key: Vec<u8>) -> SageXResult<(Vec<u8>, Vec<u8>)> {
            Self::encapsulate_bytes(&key)
        }

        /// Decapsulate with a password-wrapped seed. Returns shared-secret bytes.
        pub fn decapsulate(
            encapsulation: &KeyEncapsulation,
            password: &[u8],
            cipherbytes: &[u8],
        ) -> SageXResult<Vec<u8>> {
            let seed_bytes = AESHandler::decrypt_private_key(password, encapsulation)?;
            Self::decapsulate_raw_bytes(&seed_bytes, cipherbytes)
        }

        /// Byte-level decapsulation with a raw 64-byte seed (no password KDF).
        /// The caller obtains `seed_bytes` by decrypting its own `.prv`.
        /// Returns the raw shared-secret bytes.
        pub fn decapsulate_raw_bytes(
            seed_bytes: &[u8],
            cipherbytes: &[u8],
        ) -> SageXResult<Vec<u8>> {
            let seed_arr =
                KemSeed::try_from(seed_bytes).map_err(|_| MLKEMSecretKeyDerivationFailed)?;
            let dk = DecapsulationKey768::from_seed(seed_arr);
            let ss = dk
                .decapsulate_slice(cipherbytes)
                .map_err(|_| MLKEMCipherDerivationError)?;
            Ok(ss.as_slice().to_vec())
        }
    }
}

pub mod dsa {
    use ml_dsa::{
        KeyExport, KeyInit, Keypair, MlDsa65, Signature, SigningKey, VerifyingKey,
        signature::{SignatureEncoding, Signer, Verifier},
    };

    use crate::{
        aes::{AESHandler, KeyEncapsulation},
        error::{
            SageXCryptoError::{
                MLDSAPublicKeyDerivationFailed, MLDSASecretKeyDerivationFailed,
                MLDSASignatureVerificationFailed,
            },
            SageXResult,
        },
    };

    /// ML-DSA-65 seed length (bytes).
    #[allow(dead_code)]
    pub const SEED_LEN: usize = 32;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct KeyGen;

    impl KeyGen {
        pub fn generate_from_password(password: &[u8]) -> SageXResult<(KeyEncapsulation, Vec<u8>)> {
            use ml_dsa::Generate;
            let sk = SigningKey::<MlDsa65>::generate();
            let seed = sk.to_seed();
            let vk_bytes = sk.verifying_key().to_bytes().to_vec();
            let encapsulation = AESHandler::encrypt_private_key(seed.as_slice(), password)?;
            Ok((encapsulation, vk_bytes))
        }
    }

    pub struct Implement;

    impl Implement {
        /// Sign with a password-wrapped seed. Returns signature bytes.
        ///
        /// NOTE: the old `context: &[u8]` parameter is gone — the `ml-dsa`
        /// `Signer` impl is deterministic with an empty context per FIPS 204
        /// default. Callers that need domain separation must hash it into
        /// `message` themselves.
        pub fn sign_from_password(
            encapsulation: &KeyEncapsulation,
            password: &[u8],
            message: &[u8],
        ) -> SageXResult<Vec<u8>> {
            let seed_bytes = AESHandler::decrypt_private_key(password, encapsulation)?;
            let seed_arr = ml_dsa::Seed::try_from(&seed_bytes[..])
                .map_err(|_| MLDSASecretKeyDerivationFailed)?;
            let sk = SigningKey::<MlDsa65>::new(&seed_arr);
            Ok(sk.sign(message).to_bytes().to_vec())
        }
    }

    /// Byte-level signature verification (no `ml-dsa` types cross the API).
    pub fn verify_signature_bytes(
        key: &[u8],
        message: &[u8],
        signature_bytes: &[u8],
    ) -> SageXResult<()> {
        let vk = VerifyingKey::<MlDsa65>::new_from_slice(key)
            .map_err(|_| MLDSAPublicKeyDerivationFailed)?;
        let sig = Signature::<MlDsa65>::try_from(signature_bytes)
            .map_err(|_| MLDSASignatureVerificationFailed)?;
        vk.verify(message, &sig)
            .map_err(|_| MLDSASignatureVerificationFailed)
    }
}
