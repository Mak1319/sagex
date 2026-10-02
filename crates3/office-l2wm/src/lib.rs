pub mod archive;
pub mod central;
pub mod eocd;
pub mod error;
pub mod extra;
pub mod flags;
pub mod format;
pub mod local;
pub mod watermark;
#[cfg(feature = "tree")]
pub mod tree;

pub use archive::ZipArchive;
pub use central::CentralDirectoryFileHeader;
pub use eocd::{Eocd, Eocd32, Eocd64, Eocd64Locator, EocdFinder};
pub use error::{Error, OL2WMResult};
pub use extra::{ExtraField, ExtraFieldValue, ExtraFields};
pub use flags::{CompressionMethod, DosDate, DosTime, GeneralPurposeBitFlags};
pub use local::LocalFileHeader;
pub use watermark::{ContainerKind, FontWatermark};

pub const EOCD: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
