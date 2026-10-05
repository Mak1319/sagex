pub enum Error {
    AesKeyDerivationError,
    AesNonceDerivationError,
    AesEncryptionError,
    AesDecryptionError,
    KemKeyDerivationError,
    KemKeyNotProvided,
    DsaKeyDerivationError,
    DsaKeyVerificationError,
    DsaKeyNotProvided,
}

pub type CResult<T> = Result<T, Error>;
