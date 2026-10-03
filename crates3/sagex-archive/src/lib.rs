use std::io::{Cursor, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use sagex_capsule::format::{
    CountedBytes, DekEntry, EOCD_LEN, Eocd, FLAG_ENCRYPTED, FLAG_HAS_PERMS, FileRecord,
};
use sagex_crypto::aes::DekSealer;

/// This is how many plain bytes go in one chunk before locking.
///
/// What it does it keeps memory small: big files move piece by piece
/// and every piece can be opened without touching the others.
pub const CHUNK_BYTES: usize = 65536;

/// Storage method numbers kept on the wire.
pub const METHOD_STORED: u8 = 0;
pub const METHOD_DEFLATED: u8 = 1;

/// This is one plain file waiting to go inside the archive.
///
/// What it does it holds the path the file will have inside the archive
/// with its bytes and its unix owners, so the builder needs nothing else.
#[derive(Debug, Clone)]
pub struct ArchiveFile {
    pub relative_path: String,
    pub file_bytes: Vec<u8>,
    pub permission_mode: u32,
    pub owner_user_id: u32,
    pub owner_group_id: u32,
}

/// This is the failure of building or opening an archive.
#[derive(Debug)]
pub enum ArchiveError {
    InputOutput(String),
    Crypto(String),
    Encoding(String),
    KeyFile(String),
    ChecksumMismatch(String),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ArchiveError {}

/// This walks an input folder and collects every plain file under it.
///
/// What it does it goes through folders inside folders and keeps each file
/// with its path relative to the root, using forward slashes, so the
/// archive looks the same on every machine.
pub fn walk_input_directory(root_folder: &Path) -> Result<Vec<ArchiveFile>, ArchiveError> {
    let mut found_files = Vec::new();
    let mut folders_to_visit = vec![root_folder.to_path_buf()];
    while let Some(current_folder) = folders_to_visit.pop() {
        let folder_entries = std::fs::read_dir(&current_folder)
            .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
        for folder_entry in folder_entries {
            let entry_path = folder_entry
                .map_err(|error| ArchiveError::InputOutput(error.to_string()))?
                .path();
            if entry_path.is_dir() {
                folders_to_visit.push(entry_path);
                continue;
            }
            let relative_path = entry_path
                .strip_prefix(root_folder)
                .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
            let mut forward_path = String::new();
            for (piece_index, piece) in relative_path.components().enumerate() {
                if piece_index > 0 {
                    forward_path.push('/');
                }
                forward_path.push_str(&piece.as_os_str().to_string_lossy());
            }
            let file_bytes = std::fs::read(&entry_path)
                .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
            let file_metadata = std::fs::metadata(&entry_path)
                .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
            #[cfg(unix)]
            let permission_mode =
                std::os::unix::fs::PermissionsExt::mode(&file_metadata.permissions());
            #[cfg(not(unix))]
            let permission_mode = 0o644u32;
            found_files.push(ArchiveFile {
                relative_path: forward_path,
                file_bytes,
                permission_mode,
                owner_user_id: rustix::process::getuid().as_raw(),
                owner_group_id: rustix::process::getgid().as_raw(),
            });
        }
    }
    found_files.sort_by(|first_file, second_file| {
        first_file.relative_path.cmp(&second_file.relative_path)
    });
    Ok(found_files)
}

/// This squeezes plain bytes smaller with raw deflate.
///
/// What it does it locks nothing, it only makes the bytes shorter so the
/// archive stays small.
pub fn compress_file_bytes(plain_bytes: &[u8]) -> Result<Vec<u8>, ArchiveError> {
    use std::io::Write;
    let mut squeezer = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
    squeezer
        .write_all(plain_bytes)
        .map_err(|error| ArchiveError::Encoding(error.to_string()))?;
    squeezer
        .finish()
        .map_err(|error| ArchiveError::Encoding(error.to_string()))
}
/// This blows squeezed bytes back to plain bytes.
///
/// What it does it undoes what the squeeze function did, nothing more.
pub fn decompress_file_bytes(squeezed_bytes: &[u8]) -> Result<Vec<u8>, ArchiveError> {
    use std::io::Read;
    let mut blower = flate2::read::DeflateDecoder::new(squeezed_bytes);
    let mut plain_bytes = Vec::new();
    blower
        .read_to_end(&mut plain_bytes)
        .map_err(|error| ArchiveError::Encoding(error.to_string()))?;
    Ok(plain_bytes)
}

/// This mixes the chunk number into the base nonce.
///
/// What it does it gives every chunk its own nonce from one file nonce,
/// so the same key never locks two chunks with the same nonce.
pub fn derive_chunk_nonce(base_nonce: &[u8; 12], chunk_index: u64) -> [u8; 12] {
    let mut mixed_nonce = *base_nonce;
    for (slot_index, index_byte) in chunk_index.to_le_bytes().iter().enumerate() {
        mixed_nonce[4 + slot_index] ^= *index_byte;
    }
    mixed_nonce
}

/// This counts the checksum of plain bytes.
///
/// What it does it makes one number from the bytes so a changed file is
/// found when opening the archive later.
pub fn checksum_plain_bytes(plain_bytes: &[u8]) -> u32 {
    let mut checksum_maker = crc32fast::Hasher::new();
    checksum_maker.update(plain_bytes);
    checksum_maker.finalize()
}

/// This builds one locked file record from a plain file.
///
/// What it does it squeezes the bytes, cuts them into chunks, locks every
/// chunk with a mixed nonce, locks the file name with its own nonce, and
/// fills all the length fields so the record writes straight to the wire.
pub fn build_file_record(
    archive_file: &ArchiveFile,
    archive_key: &DekSealer,
    file_unique_nonce: [u8; 12],
) -> Result<FileRecord, ArchiveError> {
    let squeezed_bytes = compress_file_bytes(&archive_file.file_bytes)?;
    let (file_name_nonce, encrypted_file_name) = archive_key
        .seal_bytes(archive_file.relative_path.as_bytes())
        .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
    let mut locked_chunks = Vec::new();
    for (chunk_index, plain_piece) in squeezed_bytes.chunks(CHUNK_BYTES).enumerate() {
        let mixed_nonce = derive_chunk_nonce(&file_unique_nonce, chunk_index as u64);
        let locked_piece = archive_key
            .seal_bytes_with(&mixed_nonce, plain_piece)
            .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
        locked_chunks.push(CountedBytes::from(locked_piece));
    }
    let encrypted_name = encrypted_file_name;
    Ok(FileRecord {
        record_flags: FLAG_ENCRYPTED | FLAG_HAS_PERMS,
        file_name_nonce,
        file_name_length: encrypted_name.len() as u16,
        encrypted_file_name: encrypted_name,
        storage_method: METHOD_DEFLATED,
        chunk_size_in_bytes: CHUNK_BYTES as u32,
        chunk_count: locked_chunks.len() as u32,
        file_unique_nonce,
        posix_permission_mode: archive_file.permission_mode,
        owner_user_id: archive_file.owner_user_id,
        owner_group_id: archive_file.owner_group_id,
        data_chunks: locked_chunks,
        signature_length: 0,
        signature_bytes: Vec::new(),
        error_correction_length: 0,
        error_correction_bytes: Vec::new(),
        expected_plaintext_checksum: checksum_plain_bytes(&archive_file.file_bytes),
    })
}

/// This opens one locked file record back to a plain file.
///
/// What it does it unlocks the name, unlocks every chunk with its mixed
/// nonce, blows the bytes back up and checks the checksum matches.
pub fn open_file_record(
    file_record: &FileRecord,
    archive_key: &DekSealer,
) -> Result<ArchiveFile, ArchiveError> {
    let name_bytes = archive_key
        .open_bytes(
            &file_record.file_name_nonce,
            &file_record.encrypted_file_name,
        )
        .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
    let relative_path =
        String::from_utf8(name_bytes).map_err(|error| ArchiveError::Encoding(error.to_string()))?;
    let mut squeezed_bytes = Vec::new();
    for (chunk_index, locked_chunk) in file_record.data_chunks.iter().enumerate() {
        let mixed_nonce = derive_chunk_nonce(&file_record.file_unique_nonce, chunk_index as u64);
        let plain_piece = archive_key
            .open_bytes(&mixed_nonce, &locked_chunk.blob_bytes)
            .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
        squeezed_bytes.extend_from_slice(&plain_piece);
    }
    let file_bytes = decompress_file_bytes(&squeezed_bytes)?;
    if checksum_plain_bytes(&file_bytes) != file_record.expected_plaintext_checksum {
        return Err(ArchiveError::ChecksumMismatch(relative_path));
    }
    Ok(ArchiveFile {
        relative_path,
        file_bytes,
        permission_mode: file_record.posix_permission_mode,
        owner_user_id: file_record.owner_user_id,
        owner_group_id: file_record.owner_group_id,
    })
}

/// This writes one magic number at the end of a growing byte buffer.
///
/// What it does it marks the start of a header, a record or the end
/// header, because the read side checks the magic but never writes it.
pub fn write_magic_bytes(buffer: &mut Vec<u8>, magic_number: u32) {
    buffer.extend_from_slice(&magic_number.to_le_bytes());
}

/// This writes one value at the end of a growing byte buffer in little endian.
///
/// What it does it keeps all the wire writing in one place. Give it a
/// closure which writes the value; the start offset comes back.
pub fn write_wire_value(
    buffer: &mut Vec<u8>,
    write_step: impl FnOnce(&mut Cursor<Vec<u8>>) -> binrw::BinResult<()>,
) -> Result<u64, ArchiveError> {
    let mut scratch_cursor = Cursor::new(Vec::new());
    write_step(&mut scratch_cursor).map_err(|error| ArchiveError::Encoding(error.to_string()))?;
    let start_offset = buffer.len() as u64;
    buffer.extend(scratch_cursor.into_inner());
    Ok(start_offset)
}

/// This reads one value from archive bytes at the given offset.
///
/// What it does it keeps all the wire reading in one place. Give it a
/// closure which reads the value; the value comes back.
pub fn read_wire_value<T>(
    archive_bytes: &[u8],
    offset: u64,
    read_step: impl FnOnce(&mut Cursor<&[u8]>) -> binrw::BinResult<T>,
) -> Result<T, ArchiveError> {
    let mut cursor = Cursor::new(archive_bytes);
    cursor
        .seek(SeekFrom::Start(offset))
        .map_err(|error| ArchiveError::Encoding(error.to_string()))?;
    read_step(&mut cursor).map_err(|error| ArchiveError::Encoding(error.to_string()))
}

/// This builds one DEK table line from a wrapped packet.
///
/// What it does it copies the nonce and the locked bytes with the user
/// name, filling the length fields so the line writes straight.
pub fn build_dek_entry(
    username: String,
    packet_nonce: [u8; 12],
    wrapped_dek_bytes: Vec<u8>,
    kem_ciphertext_bytes: Vec<u8>,
) -> DekEntry {
    DekEntry {
        username_length: username.len() as u16,
        username,
        wrapping_nonce: packet_nonce,
        wrapped_dek_length: wrapped_dek_bytes.len() as u32,
        wrapped_dek_bytes,
        kem_ciphertext_length: kem_ciphertext_bytes.len() as u32,
        kem_ciphertext_bytes,
    }
}

/// This reads the end header from the tail of archive bytes.
///
/// What it does it jumps to the last fixed bytes and opens them, so the
/// reader learns where the map and the key table live.
pub fn read_end_header(archive_bytes: &[u8]) -> Result<Eocd, ArchiveError> {
    use binrw::{BinRead, Endian};
    if archive_bytes.len() < EOCD_LEN {
        return Err(ArchiveError::Encoding(
            "archive is shorter than its end header".to_string(),
        ));
    }
    read_wire_value(
        archive_bytes,
        (archive_bytes.len() - EOCD_LEN) as u64,
        |c| Eocd::read_options(c, Endian::Little, ()),
    )
}

/// This writes a plain file back to disk with its unix permissions.
///
/// What it does it makes the folders it needs, writes the bytes and sets
/// the mode bits the file had when it went in.
pub fn write_restored_file(
    output_root: &Path,
    archive_file: &ArchiveFile,
) -> Result<PathBuf, ArchiveError> {
    let output_path = output_root.join(&archive_file.relative_path);
    if let Some(parent_folder) = output_path.parent() {
        std::fs::create_dir_all(parent_folder)
            .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
    }
    std::fs::write(&output_path, &archive_file.file_bytes)
        .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            &output_path,
            std::fs::Permissions::from_mode(archive_file.permission_mode),
        )
        .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
    }
    Ok(output_path)
}
