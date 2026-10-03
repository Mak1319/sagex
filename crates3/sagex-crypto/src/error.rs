#[derive(Debug)]
pub enum SagexCrypotError {
    AESKeyDerivationError,
    NonceDerivationError,
    AESEncryptionError,
    AESDecryptionError,
    KEMEncapsulationError,
    KEMDecapsulationError,
    SignatureVerifyError,
    SignatureSignError,
    StreamReadError,
}

pub type SResult<T> = Result<T, SagexCrypotError>;
