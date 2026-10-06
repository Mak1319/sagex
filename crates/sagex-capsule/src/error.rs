pub enum CapsuleError {
    EncryptionError(sagex_crypto::error::Error),
    DecryptionError(sagex_crypto::error::Error),
    NonceUnprovided,
}
