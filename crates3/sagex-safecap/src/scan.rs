use std::io::{Read, Seek, SeekFrom};

use sagex_capsule::format::{
    ARCHIVE_SIG_LEN, EOCD_LEN, FLAG_ARCHIVE_SIG, FLAG_HAS_DEK_TABLE, HEADER_LEN, MAGIC, REC_MAGIC,
    VERSION,
};

use crate::DEFAULT_BUDGET;

/// This is the memory budget for one full read.
///
/// What it does it draws the line: predictions at or under it pass
/// straight through, predictions above it need signer proof first.
#[derive(Debug, Clone)]
pub struct Budget {
    pub maximum_allowed_bytes: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            maximum_allowed_bytes: DEFAULT_BUDGET,
        }
    }
}

/// This is what one stored file claims to need.
///
/// What it does it holds sizes and the file offset without holding any
/// file bytes, so adding up claims never costs the claimed memory.
#[derive(Debug, Clone, Default)]
pub struct RecordClaim {
    pub record_offset: u64,
    pub record_length: u64,
    pub chunks_claimed: u64,
    pub signature_length: u64,
    pub error_correction_length: u64,
}

/// This is the end header as read from the tail.
///
/// What it does it anchors the whole scan: every other offset in the file
/// is checked against what this header promises.
#[derive(Debug, Clone, Default)]
pub struct EocdInfo {
    pub central_directory_offset: u64,
    pub central_directory_length: u64,
    pub dek_table_offset: u64,
    pub dek_table_length: u64,
    pub stored_file_count: u32,
    pub archive_flags: u16,
    pub archive_is_signed: bool,
}

/// This is the whole-file memory prediction.
///
/// What it does it adds up every claimed byte in the file — records,
/// map, key table and signature — while holding claims only, never the
/// file bytes themselves.
#[derive(Debug, Clone, Default)]
pub struct Prediction {
    pub record_claims: Vec<RecordClaim>,
    pub central_directory_bytes: u64,
    pub dek_table_bytes: u64,
    pub archive_signature_bytes: u64,
    pub total_claimed_bytes: u64,
    pub end_header: EocdInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    Io,
    NotCapsule,
    UnsupportedVersion,
    Corrupt,
    Overflow,
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ScanError {}

fn checked_add_file_sizes(first_size: u64, second_size: u64) -> Result<u64, ScanError> {
    first_size.checked_add(second_size).ok_or(ScanError::Overflow)
}

struct FileWalker<'a, Reader: Read + Seek> {
    file_reader: &'a mut Reader,
    file_length_in_bytes: u64,
    predicted_total_bytes: u64,
}

impl<Reader: Read + Seek> FileWalker<'_, Reader> {
    fn seek_to_offset(&mut self, file_offset: u64) -> Result<(), ScanError> {
        if file_offset > self.file_length_in_bytes {
            return Err(ScanError::Corrupt);
        }
        self.file_reader
            .seek(SeekFrom::Start(file_offset))
            .map_err(|_| ScanError::Io)?;
        Ok(())
    }

    fn read_u16_from_file(&mut self) -> Result<u16, ScanError> {
        let mut byte_buffer = [0u8; 2];
        self.file_reader.read_exact(&mut byte_buffer).map_err(|_| ScanError::Io)?;
        Ok(u16::from_le_bytes(byte_buffer))
    }

    fn read_u32_from_file(&mut self) -> Result<u32, ScanError> {
        let mut byte_buffer = [0u8; 4];
        self.file_reader.read_exact(&mut byte_buffer).map_err(|_| ScanError::Io)?;
        Ok(u32::from_le_bytes(byte_buffer))
    }

    fn read_u64_from_file(&mut self) -> Result<u64, ScanError> {
        let mut byte_buffer = [0u8; 8];
        self.file_reader.read_exact(&mut byte_buffer).map_err(|_| ScanError::Io)?;
        Ok(u64::from_le_bytes(byte_buffer))
    }

    fn skip_ahead_by_bytes(&mut self, byte_count: u64) -> Result<(), ScanError> {
        let current_file_position = self.current_file_position()?;
        self.seek_to_offset(checked_add_file_sizes(current_file_position, byte_count)?)
    }

    fn current_file_position(&mut self) -> Result<u64, ScanError> {
        self.file_reader.stream_position().map_err(|_| ScanError::Io)
    }

    fn add_to_predicted_total(&mut self, byte_count: u64) -> Result<(), ScanError> {
        self.predicted_total_bytes = checked_add_file_sizes(self.predicted_total_bytes, byte_count)?;
        Ok(())
    }
}

/// This is the scanner which measures a capsule file without loading it.
///
/// What it does it walks header, tail header, central map, key table and
/// record headers with seeks only, adding up every claimed byte size.
/// Nothing but fixed-size fields is ever read; data blobs are skipped over.
pub fn scan<Reader: Read + Seek>(
    file_reader: &mut Reader,
    file_length_in_bytes: u64,
) -> Result<Prediction, ScanError> {
    if file_length_in_bytes < (HEADER_LEN + EOCD_LEN) as u64 {
        return Err(ScanError::Corrupt);
    }
    let mut file_walker = FileWalker {
        file_reader,
        file_length_in_bytes,
        predicted_total_bytes: 0,
    };
    // Fixed framing is always resident on a full read.
    file_walker.add_to_predicted_total((HEADER_LEN + EOCD_LEN) as u64)?;
    // Header at offset zero.
    file_walker.seek_to_offset(0)?;
    if file_walker.read_u32_from_file()? != MAGIC {
        return Err(ScanError::NotCapsule);
    }
    if file_walker.read_u32_from_file()? != VERSION as u32 {
        return Err(ScanError::UnsupportedVersion);
    }
    // Tail header first: it anchors every other offset.
    file_walker.seek_to_offset(file_length_in_bytes - EOCD_LEN as u64)?;
    if file_walker.read_u32_from_file()? != MAGIC {
        return Err(ScanError::Corrupt);
    }
    if file_walker.read_u16_from_file()? != VERSION {
        return Err(ScanError::UnsupportedVersion);
    }
    let archive_flags = file_walker.read_u16_from_file()?;
    let stored_file_count = file_walker.read_u32_from_file()?;
    let central_directory_offset = file_walker.read_u64_from_file()?;
    let central_directory_length = file_walker.read_u64_from_file()?;
    let dek_table_offset = file_walker.read_u64_from_file()?;
    let dek_table_length = file_walker.read_u64_from_file()?;
    for (region_offset, region_length) in [
        (central_directory_offset, central_directory_length),
        (dek_table_offset, dek_table_length),
    ] {
        if checked_add_file_sizes(region_offset, region_length)? > file_length_in_bytes {
            return Err(ScanError::Corrupt);
        }
    }
    let dek_table_is_present = archive_flags & FLAG_HAS_DEK_TABLE != 0;
    let archive_is_signed = archive_flags & FLAG_ARCHIVE_SIG != 0;
    // Central map: count plus opaque offset entries.
    file_walker.seek_to_offset(central_directory_offset)?;
    let central_entry_count = file_walker.read_u16_from_file()? as u64;
    if central_entry_count != stored_file_count as u64 {
        return Err(ScanError::Corrupt);
    }
    file_walker.skip_ahead_by_bytes(2)?;
    let mut central_entry_offsets: Vec<(u64, u64)> = Vec::new();
    for _ in 0..central_entry_count {
        let record_offset = file_walker.read_u64_from_file()?;
        let record_length = file_walker.read_u64_from_file()?;
        if checked_add_file_sizes(record_offset, record_length)? > file_length_in_bytes {
            return Err(ScanError::Corrupt);
        }
        central_entry_offsets.push((record_offset, record_length));
    }
    let central_map_end = file_walker.current_file_position()?;
    if central_map_end - central_directory_offset != central_directory_length {
        return Err(ScanError::Corrupt);
    }
    file_walker.add_to_predicted_total(central_directory_length)?;
    // Archive signature trailer, if flagged.
    let mut archive_signature_bytes = 0u64;
    if archive_is_signed {
        let signature_end = checked_add_file_sizes(
            checked_add_file_sizes(central_directory_offset, central_directory_length)?,
            ARCHIVE_SIG_LEN,
        )?;
        if signature_end > file_length_in_bytes {
            return Err(ScanError::Corrupt);
        }
        archive_signature_bytes = ARCHIVE_SIG_LEN;
        file_walker.add_to_predicted_total(ARCHIVE_SIG_LEN)?;
    }
    // Key table, if flagged.
    let mut dek_table_claimed_bytes = 0u64;
    if dek_table_is_present {
        file_walker.seek_to_offset(dek_table_offset)?;
        let dek_table_start = dek_table_offset;
        let recipient_count = file_walker.read_u32_from_file()? as u64;
        // Each entry costs at least 22 bytes on the wire; bound the loop.
        let bytes_left_in_file = file_length_in_bytes - dek_table_start;
        if recipient_count > bytes_left_in_file / 22 {
            return Err(ScanError::Corrupt);
        }
        for _ in 0..recipient_count {
            let username_length = file_walker.read_u16_from_file()? as u64;
            file_walker.skip_ahead_by_bytes(username_length)?;
            file_walker.skip_ahead_by_bytes(12)?;
            let wrapped_dek_length = file_walker.read_u32_from_file()? as u64;
            file_walker.skip_ahead_by_bytes(wrapped_dek_length)?;
            let kem_ciphertext_length = file_walker.read_u32_from_file()? as u64;
            file_walker.skip_ahead_by_bytes(kem_ciphertext_length)?;
        }
        let dek_table_end = file_walker.current_file_position()?;
        if dek_table_end - dek_table_start != dek_table_length {
            return Err(ScanError::Corrupt);
        }
        dek_table_claimed_bytes = dek_table_length;
        file_walker.add_to_predicted_total(dek_table_length)?;
    }
    // Records via central offsets: headers only, data blobs skipped.
    let mut record_claims = Vec::new();
    for (record_offset, record_length) in central_entry_offsets {
        let record_claim = scan_single_record(&mut file_walker, record_offset, record_length)?;
        file_walker.add_to_predicted_total(record_claim.record_length)?;
        record_claims.push(record_claim);
    }
    Ok(Prediction {
        record_claims,
        central_directory_bytes: central_directory_length,
        dek_table_bytes: dek_table_claimed_bytes,
        archive_signature_bytes,
        total_claimed_bytes: file_walker.predicted_total_bytes,
        end_header: EocdInfo {
            central_directory_offset,
            central_directory_length,
            dek_table_offset,
            dek_table_length,
            stored_file_count,
            archive_flags,
            archive_is_signed,
        },
    })
}

fn scan_single_record<Reader: Read + Seek>(
    file_walker: &mut FileWalker<Reader>,
    record_offset: u64,
    record_length: u64,
) -> Result<RecordClaim, ScanError> {
    file_walker.seek_to_offset(record_offset)?;
    if file_walker.read_u32_from_file()? != REC_MAGIC {
        return Err(ScanError::Corrupt);
    }
    file_walker.skip_ahead_by_bytes(2 + 12)?;
    let file_name_length = file_walker.read_u16_from_file()? as u64;
    file_walker.skip_ahead_by_bytes(file_name_length)?;
    file_walker.skip_ahead_by_bytes(1 + 4)?;
    let chunk_count = file_walker.read_u32_from_file()?;
    file_walker.skip_ahead_by_bytes(12 + 12)?;
    // Every step below first proves the needed bytes lie inside the record,
    // so the loop is bounded by the record itself (4 bytes per chunk minimum).
    let record_end = checked_add_file_sizes(record_offset, record_length)?;
    let mut chunks_claimed_bytes = 0u64;
    for _ in 0..chunk_count {
        let current_file_position = file_walker.current_file_position()?;
        if checked_add_file_sizes(current_file_position, 4)? > record_end {
            return Err(ScanError::Corrupt);
        }
        let chunk_length = file_walker.read_u32_from_file()? as u64;
        if checked_add_file_sizes(checked_add_file_sizes(current_file_position, 4)?, chunk_length)?
            > record_end
        {
            return Err(ScanError::Corrupt);
        }
        file_walker.skip_ahead_by_bytes(chunk_length)?;
        chunks_claimed_bytes = checked_add_file_sizes(chunks_claimed_bytes, chunk_length)?;
    }
    let current_file_position = file_walker.current_file_position()?;
    if checked_add_file_sizes(current_file_position, 2)? > record_end {
        return Err(ScanError::Corrupt);
    }
    let signature_length = file_walker.read_u16_from_file()? as u64;
    if checked_add_file_sizes(checked_add_file_sizes(current_file_position, 2)?, signature_length)?
        > record_end
    {
        return Err(ScanError::Corrupt);
    }
    file_walker.skip_ahead_by_bytes(signature_length)?;
    let current_file_position = file_walker.current_file_position()?;
    if checked_add_file_sizes(current_file_position, 4)? > record_end {
        return Err(ScanError::Corrupt);
    }
    let error_correction_length = file_walker.read_u32_from_file()? as u64;
    if checked_add_file_sizes(checked_add_file_sizes(current_file_position, 4)?, error_correction_length)?
        > record_end
    {
        return Err(ScanError::Corrupt);
    }
    file_walker.skip_ahead_by_bytes(error_correction_length)?;
    if checked_add_file_sizes(file_walker.current_file_position()?, 4)? != record_end {
        return Err(ScanError::Corrupt);
    }
    file_walker.skip_ahead_by_bytes(4)?;
    Ok(RecordClaim {
        record_offset,
        record_length,
        chunks_claimed: chunks_claimed_bytes,
        signature_length,
        error_correction_length,
    })
}
