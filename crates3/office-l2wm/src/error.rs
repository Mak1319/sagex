#[derive(Debug, Clone)]
pub enum Error {
    EOCDNotFound,
    BadSize { got: usize },
    BadCover { detail: &'static str },
    MissingPart { name: String },
    UnsupportedCompression { method: u16 },
    Decompress { part: String },
    Xml { stage: &'static str },
    NoWatermark,
    TooManyRecords { count: u64 },
    Encode,
    UnsupportedContainer { kind: &'static str },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EOCDNotFound => write!(f, "end of central directory not found"),
            Self::BadSize { got } => write!(f, "payload size {got} not in 4,8,16,32,64,128,256,512"),
            Self::BadCover { detail } => write!(f, "bad cover file: {detail}"),
            Self::MissingPart { name } => write!(f, "missing required part: {name}"),
            Self::UnsupportedCompression { method } => {
                write!(f, "unsupported compression method: {method}")
            }
            Self::Decompress { part } => write!(f, "failed to decompress part: {part}"),
            Self::Xml { stage } => write!(f, "XML patch failed at stage: {stage}"),
            Self::NoWatermark => write!(f, "no watermark found"),
            Self::TooManyRecords { count } => write!(f, "too many records: {count}"),
            Self::Encode => write!(f, "failed to serialize zip record"),
            Self::UnsupportedContainer { kind } => {
                write!(f, "font channel unsupported for container: {kind}")
            }
        }
    }
}

impl std::error::Error for Error {}

pub type OL2WMResult<T> = Result<T, Error>;
