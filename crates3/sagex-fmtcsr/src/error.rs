use std::fmt;

#[derive(Debug)]
pub enum CsrError {
    Encode(rkyv::rancor::Error),
    Decode(rkyv::rancor::Error),
}

impl fmt::Display for CsrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "failed to encode: {e}"),
            Self::Decode(e) => write!(f, "failed to decode: {e}"),
        }
    }
}

impl std::error::Error for CsrError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::Decode(e) => Some(e),
        }
    }
}
