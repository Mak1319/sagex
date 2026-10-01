#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SageXCryptoError {
    AESCreationError,
    NonceIterationError,
    AESEncryptionError,
    AESDecryptionError,
    MLDSASecretKeyDerivationFailed,
    MLDSASignatureDerivationFailed,
    MLDSAPublicKeyDerivationFailed,
    MLDSASignatureVerificationFailed,

    MLKEMPublicKeyDerivationFailed,
    MLKEMSecretKeyDerivationFailed,
    MLKEMCipherDerivationError,
}

pub type SageXResult<T> = Result<T, SageXCryptoError>;
