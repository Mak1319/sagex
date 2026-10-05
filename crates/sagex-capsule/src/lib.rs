use sagex_crypto::aes::NonceBytes;
use serde::{Deserialize, Serialize};

pub type MagicBytes = [u8; 4];
pub const EOCD_MAGIC_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x01];
pub const CD_MAGIC_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x02];
pub const CDFH_MAGIC_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x03];
pub const LCH_MAGIC_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x04];


#[derive(Serialize, Deserialize)]
pub struct EndOfCentralDirectory {
    pub magic_number: MagicBytes,
    pub signature: bool,
    #[serde(with = "postcard::fixint::le")]
    pub signature_offset: u64,
    #[serde(with = "postcard::fixint::le")]
    pub signature_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub central_directory_offset: u64,
    #[serde(with = "postcard::fixint::le")]
    pub central_directory_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub checksum: u32,
}


#[derive(Serialize, Deserialize)]
pub struct CentralDirectory {
    pub magic_number: MagicBytes,
    pub version_made_by: u32,
    pub version_required: u32,
    pub entry_count: u64,
    pub comment: String,
    pub entries: Vec<CentralDirectoryFileHeader>,
}

#[derive(Serialize, Deserialize)]
pub enum CompressionMethod {
    None,
    Deflate,
}

pub enum ErrorCorrectionMethod {
    None,
    ReedSolomon,
}

#[derive(Serialize, Deserialize)]
pub struct CentralDirectoryFileHeader {
    pub magic_number: MagicBytes,
    pub permission: u16,
    pub compression: CompressionMethod,
    pub encryption: bool,
    pub ecc: bool,
    #[serde(with = "postcard::fixint::le")]
    pub crc32_compressed: u32,
    #[serde(with = "postcard::fixint::le")]
    pub crc32_uncompress: u32,
    pub file_name: String,
    #[serde(with = "postcard::fixint::le")]
    pub local_file_header_offset: u64,
    #[serde(with = "postcard::fixint::le")]
    pub local_file_header_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub compressed_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub uncompressed_size: u64,
    pub comment: String,
}


#[derive(Serialize, Deserialize)]
pub struct LocalFileHeader {
    pub magic_number: MagicBytes,
    pub permission: u16,
    pub compression: CompressionMethod,
    pub encryption: bool,
    pub ecc: bool,

    #[serde(with = "postcard::fixint::le")]
    pub crc32_compressed: u32,
    #[serde(with = "postcard::fixint::le")]
    pub crc32_uncompressed: u32,
    pub file_name: String,
    #[serde(with = "postcard::fixint::le")]
    pub compressed_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub uncompressed_size: u64,
    pub comment: String,
}

#[derive(Serialize, Deserialize)]
pub struct CipherChunk {
    pub key_index: u32,
    pub buffer: Vec<u8>,
    pub nonce: NonceBytes,
    pub ecc: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct ChunkTag {
    #[serde(with = "postcard::fixint::le")]
    pub offset: u32,
    #[serde(with = "postcard::fixint::le")]
    pub size: u32,
}

#[derive(Serialize, Deserialize)]
pub struct NonCipherChunk {
    pub buffer: Vec<u8>,
    pub ecc: Vec<u8>,
}

pub struct Signature {
    pub signature: Vec<u8>,
    pub user_name: String,
}
