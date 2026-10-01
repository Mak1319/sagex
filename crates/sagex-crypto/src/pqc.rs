pub mod kem {
    use rand_core::OsRng;
    use rustpq::ml_kem_hybrid::x25519_mlkem768::{
        Ciphertext, PublicKey, SecretKey, SharedSecret, decapsulate, encapsulate, generate,
    };

    use crate::{
        aes::{AESHandler, KeyEncapsulation},
        error::{
            SageXCryptoError::{
                MLKEMCipherDerivationError, MLKEMPublicKeyDerivationFailed,
                MLKEMSecretKeyDerivationFailed,
            },
            SageXResult,
        },
    };

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct KeyGen;

    impl KeyGen {
        pub fn generate_from_password(password: &[u8]) -> SageXResult<(KeyEncapsulation, Vec<u8>)> {
            let (pk, sk) = generate(&mut OsRng);
            let encap = AESHandler::encrypt_private_key(sk.as_bytes().as_ref(), password)?;
            Ok((encap, pk.as_bytes().to_vec()))
        }
    }

    pub struct Implementation;

    impl Implementation {
        pub fn encapsulate(key: Vec<u8>) -> SageXResult<(Ciphertext, SharedSecret)> {
            let public_key =
                PublicKey::from_bytes(&key).map_err(|_| MLKEMPublicKeyDerivationFailed)?;
            let result = encapsulate(&public_key, &mut OsRng);
            Ok(result)
        }

        /// Byte-level encapsulation for envelope construction: returns
        /// `(kem_ciphertext_bytes, shared_secret_bytes)`.
        pub fn encapsulate_bytes(key: &[u8]) -> SageXResult<(Vec<u8>, Vec<u8>)> {
            let (ct, ss) = Self::encapsulate(key.to_vec())?;
            Ok((ct.as_bytes().to_vec(), ss.as_bytes().to_vec()))
        }

        pub fn decapsulate(
            encapsulation: KeyEncapsulation,
            password: &[u8],
            cipherbytes: &[u8],
        ) -> SageXResult<SharedSecret> {
            let secret_key_bytes = AESHandler::decrypt_private_key(password, encapsulation)?;
            let secret_key = SecretKey::from_bytes(&secret_key_bytes)
                .map_err(|_| MLKEMSecretKeyDerivationFailed)?;

            let cipher =
                Ciphertext::from_bytes(cipherbytes).map_err(|_| MLKEMCipherDerivationError)?;
            let result = decapsulate(&secret_key, &cipher);
            Ok(result)
        }

        /// Byte-level decapsulation with a raw secret key (no password KDF).
        /// The caller obtains `secret_bytes` by decrypting its own `.prv`.
        /// Returns the raw shared-secret bytes.
        pub fn decapsulate_raw_bytes(
            secret_bytes: &[u8],
            cipherbytes: &[u8],
        ) -> SageXResult<Vec<u8>> {
            let secret_key = SecretKey::from_bytes(secret_bytes)
                .map_err(|_| MLKEMSecretKeyDerivationFailed)?;
            let cipher =
                Ciphertext::from_bytes(cipherbytes).map_err(|_| MLKEMCipherDerivationError)?;
            Ok(decapsulate(&secret_key, &cipher).as_bytes().to_vec())
        }
    }
}

pub mod dsa {
    use rand_core::OsRng;
    use rustpq::ml_dsa::{
        mldsa65::{PublicKey, SecretKey, Signature, generate, verify},
        sign,
    };

    use crate::{
        aes::{AESHandler, KeyEncapsulation},
        error::{
            SageXCryptoError::{
                MLDSAPublicKeyDerivationFailed, MLDSASecretKeyDerivationFailed,
                MLDSASignatureDerivationFailed, MLDSASignatureVerificationFailed,
            },
            SageXResult,
        },
    };

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct KeyGen;

    impl KeyGen {
        pub fn generate_from_password(password: &[u8]) -> SageXResult<(KeyEncapsulation, Vec<u8>)> {
            let (pk, sk) = generate(&mut OsRng);
            let encapsulation = AESHandler::encrypt_private_key(sk.as_bytes().as_ref(), password)?;
            Ok((encapsulation, pk.as_bytes().to_vec()))
        }
    }

    pub struct Implement;

    impl Implement {
        pub fn sign_from_password(
            encapsulation: KeyEncapsulation,
            password: &[u8],
            message: &[u8],
            context: &[u8],
        ) -> SageXResult<Signature> {
            let sk_bytes = AESHandler::decrypt_private_key(password, encapsulation)?;
            let secret_key =
                SecretKey::from_bytes(&sk_bytes).map_err(|_| MLDSASecretKeyDerivationFailed)?;
            let signature = sign::mldsa65::sign(&secret_key, message, &context, &mut OsRng)
                .map_err(|_| MLDSASignatureDerivationFailed)?;

            Ok(signature)
        }
    }

    pub fn verify_signature(
        key: Vec<u8>,
        message: &[u8],
        context: &[u8],
        signature: Signature,
    ) -> SageXResult<()> {
        let public_key = PublicKey::from_bytes(&key).map_err(|_| MLDSAPublicKeyDerivationFailed)?;
        verify(&public_key, message, context, &signature)
            .map_err(|_| MLDSASignatureVerificationFailed)?;
        Ok(())
    }

    /// Byte-level signature verification (no rustpq types cross the API).
    pub fn verify_signature_bytes(
        key: &[u8],
        message: &[u8],
        context: &[u8],
        signature_bytes: &[u8],
    ) -> SageXResult<()> {
        let signature =
            Signature::from_bytes(signature_bytes).map_err(|_| MLDSASignatureVerificationFailed)?;
        verify_signature(key.to_vec(), message, context, signature)
    }
}
