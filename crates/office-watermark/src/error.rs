use thiserror::Error;

/// Errors returned by `office-watermark`.
#[derive(Debug, Error)]
pub enum Error {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("xml error in '{entry}': {source}")]
    Xml {
        entry: String,
        #[source]
        source: quick_xml::Error,
    },

    #[error("invalid xml in '{entry}': {reason}")]
    InvalidXml { entry: String, reason: String },

    #[error("empty input")]
    EmptyInput,

    #[error("unsupported file (not a zip-based Office document): {0}")]
    UnsupportedFormat(String),

    #[error("invalid watermark value: {0}")]
    InvalidOptions(String),
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, Error>;
