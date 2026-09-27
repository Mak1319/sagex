#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SageXCryptoError {
    AESCreationError,
    NonceIterationError,
    AESEncryptionError,
    AESDecryptionError,
}

pub type SageXResult<T> = Result<T, SageXCryptoError>;
