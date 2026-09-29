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
}
