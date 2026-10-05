use std::collections::HashMap;

use error::CapsuleError;
use sagex_crypto::aes::{EncryptionBuffer, KeyBytes, NonceBytes, key_derivation::KeyDerivation};
use serde::{Deserialize, Serialize};

use crate::error::CapsuleError::EncryptionError;

pub mod error;

pub type MagicBytes = [u8; 4];
pub const EOCD_MAGIC_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x01];
pub const CD_MAGIC_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x02];
pub const CDFH_MAGIC_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x03];
pub const LCH_MAGIC_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x04];
pub const SIGNATUR_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x05];
pub const DEK_TABLE_FORMAT: MagicBytes = [0x5A, 0x6E, 0x10, 0x06];

pub const CURRENT_VERSION: u32 = 1;

pub const CHUNK_SIZE: u64 = 4 * 1024 * 1024; // 4 MB 
pub const DATA_SHARDS: usize = 200;
pub const PARITY_SHARDS: usize = 15;


///  This is the identifier of the file and it will index all of things in the file
#[derive(Serialize, Deserialize)]
pub struct EndOfCentralDirectory {
    pub magic_number: MagicBytes,

    #[serde(with = "postcard::fixint::le")]
    pub version: u32,
    // DEK table related
    pub table: bool,
    #[serde(with = "postcard::fixint::le")]
    pub table_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub table_offset: u64,

    //Signature related
    pub signature: bool,
    #[serde(with = "postcard::fixint::le")]
    pub signature_offset: u64,
    #[serde(with = "postcard::fixint::le")]
    pub signature_size: u64,

    // Central directory related
    #[serde(with = "postcard::fixint::le")]
    pub central_directory_offset: u64,
    #[serde(with = "postcard::fixint::le")]
    pub central_directory_size: u64,

    // Checksum related (SHA-256 over all bytes preceding this EOCD)
    pub checksum: [u8; 32],
}

impl Default for EndOfCentralDirectory {
    fn default() -> Self {
        EndOfCentralDirectory {
            magic_number: EOCD_MAGIC_FORMAT,
            version: CURRENT_VERSION,
            table: false,
            table_size: 0,
            table_offset: 0,
            signature: false,
            signature_offset: 0,
            signature_size: 0,
            central_directory_offset: 0,
            central_directory_size: 0,
            checksum: [0u8; 32],
        }
    }
}

/// This will index all the files
#[derive(Serialize, Deserialize)]
pub struct CentralDirectory {
    pub magic_number: MagicBytes,
    pub version_made_by: u32,
    pub version_required: u32,
    pub entry_count: u64,
    pub comment: String,
    pub entries: Vec<CentralDirectoryFileHeader>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum CompressionMethod {
    None,
    Deflate,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum ErrorCorrectionMethod {
    None,
    ReedSolomon,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CentralDirectoryFileHeader {
    pub magic_number: MagicBytes,

    // Fs specific
    pub permission: u16,
    pub is_directory: bool,

    // Archive specific
    pub compression: CompressionMethod,
    pub encryption: bool,
    pub ecc: ErrorCorrectionMethod,

    // validation specific
    #[serde(with = "postcard::fixint::le")]
    pub crc32_compressed: u32,
    #[serde(with = "postcard::fixint::le")]
    pub crc32_uncompressed: u32,

    // Payload
    pub file_name: Vec<u8>,
    pub file_nonce: Option<NonceBytes>,

    // Archive location specific
    #[serde(with = "postcard::fixint::le")]
    pub local_file_header_offset: u64,
    #[serde(with = "postcard::fixint::le")]
    pub local_file_header_size: u64,

    // Size specific
    #[serde(with = "postcard::fixint::le")]
    pub compressed_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub uncompressed_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub encrypted_sized: u64,

    // Comment specific
    pub comment: String,
}


#[derive(Serialize, Deserialize)]
pub struct LocalFileHeader {
    pub magic_number: MagicBytes,

    // FS specific
    pub permission: u16,
    pub is_directory: bool,

    // Archive specific
    pub compression: CompressionMethod,
    pub encryption: bool,
    pub ecc: ErrorCorrectionMethod,

    // Validation specific
    #[serde(with = "postcard::fixint::le")]
    pub crc32_compressed: u32,
    #[serde(with = "postcard::fixint::le")]
    pub crc32_uncompressed: u32,

    // Payload
    pub file_name: Vec<u8>,
    pub file_nonce: Option<NonceBytes>,

    //Size specific
    #[serde(with = "postcard::fixint::le")]
    pub compressed_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub uncompressed_size: u64,
    #[serde(with = "postcard::fixint::le")]
    pub encrypted_size: u64,

    // Comment Specific
    pub comment: String,
}

#[derive(Serialize, Deserialize)]
pub struct CipherChunk {
    pub key_index: u32,
    pub buffer: Vec<u8>,
    pub nonce: NonceBytes,
}

#[derive(Serialize, Deserialize)]
pub struct ChunkTag {
    #[serde(with = "postcard::fixint::le")]
    pub offset: u64,
    #[serde(with = "postcard::fixint::le")]
    pub size: u64,
}

#[derive(Serialize, Deserialize)]
pub struct NonCipherChunk {
    pub buffer: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct Signature {
    pub magic: MagicBytes,
    pub signature: Vec<u8>,
    pub user_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct LicenseTable {
    pub magic_format: MagicBytes,
    pub entries: HashMap<u32, LicenseEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct LicenseEntry {
    pub key_id: u32,
    pub user_name: String,
    pub dek_entries: HashMap<String, DekEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct DekEntry {
    pub key_id: u32,
    pub kem_cipher: Vec<u8>,
    pub dek_cipher: Vec<u8>,
    pub dek_nonce: NonceBytes,
}

pub trait ToEncrypted {
    fn encrypt(&mut self, key: KeyBytes) -> Result<(), CapsuleError>;
    fn decrypt(&mut self, key: KeyBytes) -> Result<(), CapsuleError>;
}


impl From<CentralDirectoryFileHeader> for LocalFileHeader {
    fn from(value: CentralDirectoryFileHeader) -> Self {
        Self {
            magic_number: LCH_MAGIC_FORMAT,
            permission: value.permission,
            is_directory: value.is_directory,
            compression: value.compression,
            encryption: value.encryption,
            ecc: value.ecc,
            crc32_compressed: value.crc32_compressed,
            crc32_uncompressed: value.crc32_uncompressed,
            file_name: value.file_name.clone(),
            file_nonce: value.file_nonce.clone(),
            compressed_size: value.compressed_size,
            uncompressed_size: value.uncompressed_size,
            encrypted_size: value.encrypted_sized,
            comment: value.comment.clone(),
        }
    }
}

impl ToEncrypted for CentralDirectoryFileHeader {
    fn encrypt(&mut self, key: KeyBytes) -> Result<(), CapsuleError> {
        let nonce_bytes = KeyDerivation::get_nonce();
        let enc_file_name = EncryptionBuffer::encrypt(&self.file_name, key, nonce_bytes)
            .map_err(|e| EncryptionError(e))?;
        self.encryption = true;
        self.file_name = enc_file_name;
        self.file_nonce = Some(nonce_bytes);
        Ok(())
    }
    fn decrypt(&mut self, key: KeyBytes) -> Result<(), CapsuleError> {
        let Some(nonce_bytes) = self.file_nonce else {
            return Err(CapsuleError::NonceUnprovided);
        };

        let file_name = EncryptionBuffer::decrypt(&self.file_name, key, nonce_bytes)
            .map_err(|e| CapsuleError::DecryptionError(e))?;

        self.file_name = file_name;
        Ok(())
    }
}


impl Default for CentralDirectory {
    fn default() -> Self {
        CentralDirectory {
            magic_number: CD_MAGIC_FORMAT,
            version_made_by: CURRENT_VERSION,
            version_required: CURRENT_VERSION,
            entry_count: 0,
            comment: "".into(),
            entries: Vec::new(),
        }
    }
}


pub mod helper {
    use reed_solomon_erasure::galois_8::ReedSolomon;

    use crate::{DATA_SHARDS, PARITY_SHARDS};

    pub fn calculate_ecc(cipher_chunk: &[u8]) -> Result<Vec<u8>, reed_solomon_erasure::Error> {
        let rs = ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS)?;

        let shard_size = cipher_chunk.len().div_ceil(DATA_SHARDS);

        // One allocation for ALL shards.
        let mut storage = vec![0u8; (DATA_SHARDS + PARITY_SHARDS) * shard_size];

        // Copy ciphertext into the data-shard area.
        storage[..cipher_chunk.len()].copy_from_slice(cipher_chunk);

        // Create slices into the same allocation.
        let mut shards: Vec<&mut [u8]> = storage.chunks_exact_mut(shard_size).collect();

        // Calculate parity directly into the preallocated parity area.
        rs.encode(&mut shards)?;

        // ECC is already contiguous after the data shards.
        let ecc_start = DATA_SHARDS * shard_size;

        Ok(storage[ecc_start..].to_vec())
    }
}
