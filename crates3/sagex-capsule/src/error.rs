#[derive(Debug)]
pub enum Error {
    KeySaltNotMatch,
    KeyDerivationError,
    KeyDecryptionError,
    InvalidRecipientKey,
    EncapsulationError,
    DecapsulationError,
    DekWrapError,
    DekUnwrapError,
    RecordReadError,
    MalformedRecord,
    EncodeError,
}

pub type CapResult<T> = Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}
