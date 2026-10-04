use std::io::{Read, Seek, SeekFrom};

use sagex_crypto::aes::dsa_verify_feed;
use sagex_crypto::error::{SResult, SagexCrypotError};

use crate::error::{CapResult, Error};
use crate::format::{ARCHIVE_SIG_LEN, CentralDir, DekTable, FLAG_HAS_SIG, FileRecord, REC_MAGIC};

/// This is the piece size we read at a time while checking.
///
/// What it does it keeps the checking memory small no matter how big the
/// file is: only this many bytes are ever held at once.
const HASH_READING_PIECE_SIZE: usize = 65536;
/// This is the most chunks one record may claim.
///
/// What it does it stops a lying file from making us build a giant length
/// table: the table only ever holds this many small numbers.
const MAX_CHUNKS_PER_RECORD: u64 = 4_000_000;

// ---------- Walk helpers for the parse phase (small reads, no blobs) ----------

fn checked_add_file_offset(file_position: u64, forward_distance: u64) -> CapResult<u64> {
    file_position
        .checked_add(forward_distance)
        .ok_or(Error::RecordReadError)
}

fn read_u16_from_file<R: Read + Seek>(file_reader: &mut R) -> CapResult<u16> {
    let mut byte_buffer = [0u8; 2];
    file_reader
        .read_exact(&mut byte_buffer)
        .map_err(|_| Error::RecordReadError)?;
    Ok(u16::from_le_bytes(byte_buffer))
}

fn read_u32_from_file<R: Read + Seek>(file_reader: &mut R) -> CapResult<u32> {
    let mut byte_buffer = [0u8; 4];
    file_reader
        .read_exact(&mut byte_buffer)
        .map_err(|_| Error::RecordReadError)?;
    Ok(u32::from_le_bytes(byte_buffer))
}

fn seek_reader_to_offset<R: Read + Seek>(file_reader: &mut R, file_offset: u64) -> CapResult<()> {
    file_reader
        .seek(SeekFrom::Start(file_offset))
        .map_err(|_| Error::RecordReadError)?;
    Ok(())
}

fn read_exact_bytes<R: Read + Seek>(file_reader: &mut R, byte_count: usize) -> CapResult<Vec<u8>> {
    let mut byte_buffer = vec![0u8; byte_count];
    file_reader
        .read_exact(&mut byte_buffer)
        .map_err(|_| Error::RecordReadError)?;
    Ok(byte_buffer)
}

// ---------- Hash-phase helpers (streaming, nothing held) ----------

fn seek_hash_reader<R: Read + Seek>(file_reader: &mut R, file_offset: u64) -> SResult<()> {
    file_reader
        .seek(SeekFrom::Start(file_offset))
        .map_err(|_| SagexCrypotError::StreamReadError)?;
    Ok(())
}

/// This is the pump which pushes file bytes into the signature hash.
///
/// What it does it reads exactly the asked number of bytes in small pieces
/// and hands each piece to the hash, so huge files are checked without
/// holding them. A short file fails here instead of anywhere else.
fn push_file_piece_to_hash<R: Read + Seek>(
    file_reader: &mut R,
    mut bytes_left_to_read: u64,
    feed_bytes_to_hash: &mut dyn FnMut(&[u8]),
) -> SResult<()> {
    let mut piece_buffer = vec![0u8; HASH_READING_PIECE_SIZE];
    while bytes_left_to_read > 0 {
        let piece_length = HASH_READING_PIECE_SIZE.min(bytes_left_to_read as usize);
        file_reader
            .read_exact(&mut piece_buffer[..piece_length])
            .map_err(|_| SagexCrypotError::StreamReadError)?;
        feed_bytes_to_hash(&piece_buffer[..piece_length]);
        bytes_left_to_read -= piece_length as u64;
    }
    Ok(())
}

/// This is the checker for one stored file's signature.
///
/// What it does it checks the signature without holding the file data:
/// header bytes, chunk bytes and error-correction bytes flow through the
/// hash in small pieces. It gives back true when the signature is good and
/// false when the file carries no signature or a bad one; only a broken
/// structure or unreadable bytes are errors. The trailing plaintext
/// checksum is checked after decrypting, not here.
pub fn verify_file<R: Read + Seek>(
    file_reader: &mut R,
    record_offset: u64,
    sender_public_key_bytes: &[u8],
) -> CapResult<bool> {
    // Parse walk: fixed fields and lengths only, no blob allocation.
    seek_reader_to_offset(file_reader, record_offset)?;
    let mut record_header_bytes = read_exact_bytes(file_reader, 4)?;
    if u32::from_le_bytes(
        record_header_bytes[..4]
            .try_into()
            .map_err(|_| Error::MalformedRecord)?,
    ) != REC_MAGIC
    {
        return Err(Error::MalformedRecord);
    }
    record_header_bytes.extend_from_slice(&read_exact_bytes(file_reader, 2 + 12)?);
    let name_length_raw = read_exact_bytes(file_reader, 2)?;
    let file_name_length = u16::from_le_bytes([name_length_raw[0], name_length_raw[1]]) as u64;
    // Echo the exact wire bytes, not a re-encoding: the field is u16 on
    // the wire and widening it to u64 once smuggled in 6 phantom zeros.
    record_header_bytes.extend_from_slice(&name_length_raw);
    record_header_bytes
        .extend_from_slice(&read_exact_bytes(file_reader, file_name_length as usize)?);
    // method(1) + chunk_size(4) + chunk_count(4) + file_nonce(12) + mode/uid/gid(12)
    let fixed_header_tail = read_exact_bytes(file_reader, 33)?;
    let chunk_count = u32::from_le_bytes(
        fixed_header_tail[5..9]
            .try_into()
            .map_err(|_| Error::MalformedRecord)?,
    ) as u64;
    if chunk_count > MAX_CHUNKS_PER_RECORD {
        return Err(Error::MalformedRecord);
    }
    record_header_bytes.extend_from_slice(&fixed_header_tail);
    let record_flags = u16::from_le_bytes([record_header_bytes[4], record_header_bytes[5]]);
    if record_flags & FLAG_HAS_SIG == 0 {
        return Ok(false);
    }
    let mut chunk_lengths: Vec<u32> = Vec::new();
    for _ in 0..chunk_count {
        let chunk_length = read_u32_from_file(file_reader)?;
        chunk_lengths.push(chunk_length);
        let current_position = file_reader
            .stream_position()
            .map_err(|_| Error::RecordReadError)?;
        seek_reader_to_offset(
            file_reader,
            checked_add_file_offset(current_position, chunk_length as u64)?,
        )?;
    }
    let signature_length = read_u16_from_file(file_reader)? as usize;
    let signature_bytes = read_exact_bytes(file_reader, signature_length)?;
    let error_correction_length = read_u32_from_file(file_reader)? as u64;
    let error_correction_start = file_reader
        .stream_position()
        .map_err(|_| Error::RecordReadError)?;
    // Feed pass: header, chunk data, error-correction bytes — streamed, nothing held.
    let chunks_start = checked_add_file_offset(record_offset, record_header_bytes.len() as u64)?;
    let verification_outcome = dsa_verify_feed(
        sender_public_key_bytes,
        &signature_bytes,
        |feed_bytes_to_hash| {
            feed_bytes_to_hash(&record_header_bytes);
            let mut chunk_position = chunks_start;
            for chunk_length in &chunk_lengths {
                let chunk_data_position = chunk_position
                    .checked_add(4)
                    .ok_or(SagexCrypotError::StreamReadError)?;
                seek_hash_reader(file_reader, chunk_data_position)?;
                push_file_piece_to_hash(file_reader, *chunk_length as u64, feed_bytes_to_hash)?;
                chunk_position = chunk_data_position
                    .checked_add(*chunk_length as u64)
                    .ok_or(SagexCrypotError::StreamReadError)?;
            }
            seek_hash_reader(file_reader, error_correction_start)?;
            push_file_piece_to_hash(file_reader, error_correction_length, feed_bytes_to_hash)?;
            Ok(())
        },
    );
    match verification_outcome {
        Ok(()) => Ok(true),
        Err(SagexCrypotError::StreamReadError) => Err(Error::RecordReadError),
        Err(_) => Ok(false),
    }
}

/// This is the checker for the whole archive's signature.
///
/// What it does it streams the central map and the DEK table through the
/// hash without holding them, then checks the fixed trailing signature.
/// False means missing or bad signature; errors mean broken structure
/// or unreadable bytes.
pub fn verify_central<R: Read + Seek>(
    file_reader: &mut R,
    central_directory_offset: u64,
    central_directory_length: u64,
    dek_table_offset: u64,
    dek_table_length: u64,
    archive_is_signed: bool,
    sender_public_key_bytes: &[u8],
) -> CapResult<bool> {
    if !archive_is_signed {
        return Ok(false);
    }
    // Light structure check on the central header before streaming.
    seek_reader_to_offset(file_reader, central_directory_offset)?;
    let _entry_count = read_u16_from_file(file_reader)?;
    let verification_outcome = dsa_verify_feed(
        sender_public_key_bytes,
        &read_archive_signature_trailer(
            file_reader,
            central_directory_offset,
            central_directory_length,
        )?,
        |feed_bytes_to_hash| {
            seek_hash_reader(file_reader, central_directory_offset)?;
            push_file_piece_to_hash(file_reader, central_directory_length, feed_bytes_to_hash)?;
            if dek_table_length > 0 {
                seek_hash_reader(file_reader, dek_table_offset)?;
                push_file_piece_to_hash(file_reader, dek_table_length, feed_bytes_to_hash)?;
            }
            Ok(())
        },
    );
    match verification_outcome {
        Ok(()) => Ok(true),
        Err(SagexCrypotError::StreamReadError) => Err(Error::RecordReadError),
        Err(_) => Ok(false),
    }
}

/// This is the reader for the fixed archive signature.
///
/// What it does it jumps straight past the central map and takes the
/// signature bytes sitting there, since signatures are always the same
/// length and need no length field.
fn read_archive_signature_trailer<R: Read + Seek>(
    file_reader: &mut R,
    central_directory_offset: u64,
    central_directory_length: u64,
) -> CapResult<Vec<u8>> {
    seek_reader_to_offset(
        file_reader,
        checked_add_file_offset(central_directory_offset, central_directory_length)?,
    )?;
    read_exact_bytes(file_reader, ARCHIVE_SIG_LEN as usize)
}

/// Build the exact bytes a file signature covers.
///
/// What it does it lays out header fields, raw chunk data and raw ECC
/// bytes in the same order the checker feeds the hash: no signature
/// bytes, no length prefixes on chunks, no trailing checksum. Sign this
/// and [`verify_file`] passes; change any byte and it fails.
pub fn file_canonical_bytes(record: &FileRecord) -> Vec<u8> {
    let mut canonical_bytes = Vec::new();
    canonical_bytes.extend_from_slice(&REC_MAGIC.to_le_bytes());
    canonical_bytes.extend_from_slice(&record.record_flags.to_le_bytes());
    canonical_bytes.extend_from_slice(&record.file_name_nonce);
    canonical_bytes.extend_from_slice(&record.file_name_length.to_le_bytes());
    canonical_bytes.extend_from_slice(&record.encrypted_file_name);
    canonical_bytes.push(record.storage_method);
    canonical_bytes.extend_from_slice(&record.chunk_size_in_bytes.to_le_bytes());
    canonical_bytes.extend_from_slice(&record.chunk_count.to_le_bytes());
    canonical_bytes.extend_from_slice(&record.file_unique_nonce);
    canonical_bytes.extend_from_slice(&record.posix_permission_mode.to_le_bytes());
    canonical_bytes.extend_from_slice(&record.owner_user_id.to_le_bytes());
    canonical_bytes.extend_from_slice(&record.owner_group_id.to_le_bytes());
    for locked_chunk in &record.data_chunks {
        canonical_bytes.extend_from_slice(&locked_chunk.blob_bytes);
    }
    canonical_bytes.extend_from_slice(&record.error_correction_bytes);
    canonical_bytes
}

/// Build the exact bytes the archive signature covers.
///
/// What it does it concatenates the central map bytes and the DEK table
/// bytes exactly as they sit in the file. Sign this and [`verify_central`]
/// passes.
pub fn central_canonical_bytes(
    central: &CentralDir,
    dek_table: &DekTable,
) -> Result<Vec<u8>, crate::error::Error> {
    use binrw::{BinWrite, Endian};
    use std::io::Cursor;

    let mut canonical_bytes = Vec::new();
    let mut cursor = Cursor::new(Vec::new());
    central
        .write_options(&mut cursor, Endian::Little, ())
        .map_err(|_| crate::error::Error::EncodeError)?;
    canonical_bytes.extend_from_slice(cursor.get_ref());
    let mut cursor = Cursor::new(Vec::new());
    dek_table
        .write_options(&mut cursor, Endian::Little, ())
        .map_err(|_| crate::error::Error::EncodeError)?;
    canonical_bytes.extend_from_slice(cursor.get_ref());
    Ok(canonical_bytes)
}
