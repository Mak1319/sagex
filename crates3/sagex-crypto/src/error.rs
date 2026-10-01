pub enum SagexCrypotError {
    AESKeyDerivationError,
    NonceDerivationError,
    AESEncryptionError,
}

pub type SResult<T> = Result<T, SagexCrypotError>;
