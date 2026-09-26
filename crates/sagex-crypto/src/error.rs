#[derive(thiserror::Error, Debug)]
pub enum VaultError {
    #[error("key generation failed: {0}")]
    KeyGen(String),
    #[error("TPM error: {0}")]
    Tpm(String),
    #[error("PKCS#11/HSM error: {0}")]
    Pkcs(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("no backend available")]
    NoBackend,
}

pub type Result<T> = std::result::Result<T, VaultError>;
