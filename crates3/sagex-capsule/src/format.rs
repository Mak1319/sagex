use binrw::{BinRead, BinWrite};

/// This is the number every sagex file starts with, bytes 5A 6E 10 00.
///
/// What it does it tells the reader this is our file and not a zip or
/// something else. If the first four bytes are not this, the file is
/// refused straight away.
pub const MAGIC: u32 = 0x00106E5A;
/// This is the number every file record starts with.
///
/// What it does it marks where one stored file begins inside the archive,
/// so a scanner can tell records apart from plain data bytes.
pub const REC_MAGIC: u32 = 0x01106E5A;
/// This is the file format version we write.
///
/// What it does it lets a future reader refuse files it is too old to
/// understand instead of misreading them.
pub const VERSION: u16 = 1;

/// Record flag bits. Each bit says one optional piece is present.
pub const FLAG_ENCRYPTED: u16 = 0x0001;
pub const FLAG_HAS_SIG: u16 = 0x0002;
pub const FLAG_HAS_ECC: u16 = 0x0004;
pub const FLAG_HAS_PERMS: u16 = 0x0008;

/// EOCD flag bits.
pub const FLAG_HAS_DEK_TABLE: u16 = 0x0001;
/// This bit says an archive signature is present: fixed [`ARCHIVE_SIG_LEN`]
/// bytes follow the central directory. ML-DSA-44 signatures are fixed-size,
/// so the trailer needs no length prefix.
pub const FLAG_ARCHIVE_SIG: u16 = 0x0002;
/// This is how long an ML-DSA-44 signature always is on the wire.
///
/// What it does it lets the reader find the signature without a length
/// field: it is always exactly this many bytes after the central directory.
pub const ARCHIVE_SIG_LEN: u64 = 2420;

/// Fixed sizes on the wire.
pub const HEADER_LEN: usize = 16;
pub const EOCD_LEN: usize = 44;

/// This is the most entries we will ever believe from a file.
///
/// What it does it stops a lying file from making us build a giant table:
/// whatever count the file claims, we read at most this many.
pub const MAX_BELIEVED_ENTRY_COUNT: u32 = 1 << 20;

/// Infile helpers: small wire types plus the two bool-codec functions the
/// derives need. Nothing floating at module scope.
mod codec {    use super::MAX_BELIEVED_ENTRY_COUNT;
    use binrw::{BinRead, BinResult, BinWrite, Endian};
    use std::io::{Read, Seek, Write};

    /// Clamp a wire count to [`MAX_BELIEVED_ENTRY_COUNT`].
    pub fn clamp_count(claimed_count: u32) -> usize {
        claimed_count.min(MAX_BELIEVED_ENTRY_COUNT) as usize
    }

    /// This is the reader which takes file bytes in small pieces instead
    /// of holding everything at once.
    ///
    /// What it does it reads the claimed number of bytes in fixed 64 KiB
    /// pieces, so a lying length fails at end-of-file instead of eating all
    /// memory. Memory only ever holds bytes which are actually present.
    ///
    /// Use as `#[br(parse_with = streamed(length as usize))]`.
    pub fn streamed<Reader>(
        claimed_byte_length: usize,
    ) -> impl Fn(&mut Reader, Endian, ()) -> BinResult<Vec<u8>>
    where
        Reader: Read + Seek,
    {
        move |file_reader, _, _| {
            const PIECE_BYTE_LENGTH: usize = 65536;
            let mut collected_bytes = Vec::new();
            let mut bytes_left_to_read = claimed_byte_length as u64;
            let mut piece_buffer =
                vec![0u8; PIECE_BYTE_LENGTH.min(bytes_left_to_read as usize).max(1)];
            while bytes_left_to_read > 0 {
                let piece_length = PIECE_BYTE_LENGTH.min(bytes_left_to_read as usize);
                file_reader.read_exact(&mut piece_buffer[..piece_length])?;
                collected_bytes.extend_from_slice(&piece_buffer[..piece_length]);
                bytes_left_to_read -= piece_length as u64;
            }
            Ok(collected_bytes)
        }
    }

    /// This is one length-prefixed lump of bytes (`length u32` + bytes).
    ///
    /// What it does it carries one chunk of file data on the wire. The
    /// length is a real field so both directions derive; build via `From`.
    #[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
    #[brw(little)]
    pub struct CountedBytes {
        pub byte_length: u32,
        #[br(parse_with = streamed(byte_length as usize))]
        pub blob_bytes: Vec<u8>,
    }

    impl From<Vec<u8>> for CountedBytes {
        fn from(blob_bytes: Vec<u8>) -> Self {
            Self { byte_length: blob_bytes.len() as u32, blob_bytes }
        }
    }

    /// This is one byte which reads as true or false (`0` means false).
    ///
    /// What it does it keeps struct fields boolean while the wire stays one
    /// byte, so the format never wastes space on flags.
    pub fn read_bool<R: Read + Seek>(
        file_reader: &mut R,
        byte_order: Endian,
        _: (),
    ) -> BinResult<bool> {
        Ok(u8::read_options(file_reader, byte_order, ())? != 0)
    }

    /// This is one boolean written as one byte. Signature order is value,
    /// writer, endian, args (binrw 0.15 `write_with` convention).
    pub fn write_bool<W: Write + Seek>(
        is_set: &bool,
        file_writer: &mut W,
        byte_order: Endian,
        _: (),
    ) -> BinResult<()> {
        (*is_set as u8).write_options(file_writer, byte_order, ())
    }
}

use codec::{clamp_count, read_bool, streamed, write_bool};

pub use codec::CountedBytes;

/// This is the first 16 bytes of every archive.
///
/// What it does it announces the file: magic number, format version and
/// archive flags, so the reader knows what it is holding before reading
/// anything else.
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
#[br(magic = 0x00106E5Au32)]  // == MAGIC
pub struct ArchiveHeader {
    pub format_version: u32,
    pub archive_flags: u32,
    pub reserved: u32,
}

impl ArchiveHeader {
    pub fn new(archive_flags: u32) -> Self {
        Self { format_version: VERSION as u32, archive_flags, reserved: 0 }
    }
}

/// This is one stored file with everything about it.
///
/// What it does it holds a single file: its encrypted name, its data cut
/// into chunks, and the optional signature, error-correction bytes and
/// permissions. Length fields are real struct fields (binrw derive has no
/// `temp` equivalent), so keep them in sync with the data they describe.
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
#[br(magic = 0x01106E5Au32)]  // == REC_MAGIC
pub struct FileRecord {
    pub record_flags: u16,
    pub file_name_nonce: [u8; 12],
    pub file_name_length: u16,
    #[br(parse_with = streamed(file_name_length as usize))]
    pub encrypted_file_name: Vec<u8>,
    pub storage_method: u8,
    pub chunk_size_in_bytes: u32,
    pub chunk_count: u32,
    pub file_unique_nonce: [u8; 12],
    pub posix_permission_mode: u32,
    pub owner_user_id: u32,
    pub owner_group_id: u32,
    #[br(count = clamp_count(chunk_count))]
    pub data_chunks: Vec<CountedBytes>,
    pub signature_length: u16,
    #[br(parse_with = streamed(signature_length as usize))]
    pub signature_bytes: Vec<u8>,
    pub error_correction_length: u32,
    #[br(parse_with = streamed(error_correction_length as usize))]
    pub error_correction_bytes: Vec<u8>,
    pub expected_plaintext_checksum: u32,
}

/// This is one line of the DEK table: one recipient and their packet.
///
/// What it does it tells which wrapped DEK belongs to which username, so
/// a recipient can find their own packet without trying the others.
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub struct DekEntry {
    pub username_length: u16,
    #[br(parse_with = streamed(username_length as usize), try_map = String::from_utf8)]
    #[bw(map = |recipient_name: &String| recipient_name.as_bytes().to_vec())]
    pub username: String,
    pub wrapping_nonce: [u8; 12],
    pub wrapped_dek_length: u32,
    #[br(parse_with = streamed(wrapped_dek_length as usize))]
    pub wrapped_dek_bytes: Vec<u8>,
    pub kem_ciphertext_length: u32,
    #[br(parse_with = streamed(kem_ciphertext_length as usize))]
    pub kem_ciphertext_bytes: Vec<u8>,
}

/// This is the table which holds every recipient's packet.
///
/// What it does it collects all the wrapped DEKs in one place near the end
/// of the file, so the reader finds keys without scanning file data.
#[derive(Debug, Clone, PartialEq, Eq, Default, BinRead, BinWrite)]
#[brw(little)]
pub struct DekTable {
    pub entry_count: u32,
    #[br(count = clamp_count(entry_count))]
    pub recipient_entries: Vec<DekEntry>,
}

/// This is one line of the central map: where one file lives.
///
/// What it does it holds only an offset and a length, no names, so listing
/// the archive reveals nothing about what is inside.
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub struct CentralEntry {
    pub record_offset: u64,
    pub record_length: u64,
}

/// This is the map of the whole archive which lives near the end of the file.
///
/// What it does it holds only offsets so the reader can jump straight to
/// any file without opening the others, and it keeps names hidden. It also
/// says whether a DEK table is present at all.
#[derive(Debug, Clone, PartialEq, Eq, Default, BinRead, BinWrite)]
#[brw(little)]
pub struct CentralDir {
    pub entry_count: u16,
    #[br(parse_with = read_bool)]
    #[bw(write_with = write_bool)]
    pub has_dek_table: bool,
    pub reserved: u8,
    #[br(count = clamp_count(entry_count as u32))]
    pub record_entries: Vec<CentralEntry>,
}

/// This is the last header of the file with a fixed size.
///
/// What it does it tells the reader where everything is: how many files
/// there are and where the map and the DEK table start and how long they
/// are. The reader reads these 44 bytes first and then jumps straight to
/// what it needs. Always [`EOCD_LEN`] bytes on the wire:
/// 4 + 2 + 2 + 4 + 8 + 8 + 8 + 8 = 44.
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
#[br(magic = 0x00106E5Au32)]  // == MAGIC
pub struct Eocd {
    pub version: u16,
    pub flags: u16,
    pub stored_file_count: u32,
    pub central_directory_offset: u64,
    pub central_directory_length: u64,
    pub dek_table_offset: u64,
    pub dek_table_len: u64,
}

impl Eocd {
    pub fn new(flags: u16, stored_file_count: u32) -> Self {
        Self {
            version: VERSION,
            flags,
            stored_file_count,
            central_directory_offset: 0,
            central_directory_length: 0,
            dek_table_offset: 0,
            dek_table_len: 0,
        }
    }

    pub fn has_dek_table(&self) -> bool {
        self.flags & FLAG_HAS_DEK_TABLE != 0
    }
}
