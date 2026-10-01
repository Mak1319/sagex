#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum SageXCryptoError {
    #[error("AES instance creation failed")]
    AESCreationError,
    #[error("nonce iteration failed")]
    NonceIterationError,
    #[error("AES encryption failed")]
    AESEncryptionError,
    #[error("AES decryption failed")]
    AESDecryptionError,
    #[error("ML-DSA secret key derivation failed")]
    MLDSASecretKeyDerivationFailed,
    #[error("ML-DSA signature derivation failed")]
    MLDSASignatureDerivationFailed,
    #[error("ML-DSA public key derivation failed")]
    MLDSAPublicKeyDerivationFailed,
    #[error("ML-DSA signature verification failed")]
    MLDSASignatureVerificationFailed,
    #[error("ML-KEM public key derivation failed")]
    MLKEMPublicKeyDerivationFailed,
    #[error("ML-KEM secret key derivation failed")]
    MLKEMSecretKeyDerivationFailed,
    #[error("ML-KEM ciphertext derivation failed")]
    MLKEMCipherDerivationError,
    #[error("codec encode/decode failed")]
    CodecError,
    #[error("RNG failure")]
    RngError,
}

pub type SageXResult<T> = Result<T, SageXCryptoError>;
