use binrw::{BinRead, BinWrite};
use modular_bitfield::{bitfield, specifiers::B2};

#[bitfield]
#[derive(Debug, Clone, Copy, BinWrite)]
pub struct GeneralPurposeBitFlags {
    pub encrypted: bool,
    pub compression_option: B2,
    pub data_descriptor: bool,
    pub enhanced_deflating: bool,
    pub patched_data: bool,
    pub strong_encryption: bool,
    pub filename_utf8: bool,
    pub reserved0: bool,
    pub cd_encrypted: bool,
    pub reserved1: B2,
    pub reserved2: B2,
    pub reserved3: B2,
}

impl BinRead for GeneralPurposeBitFlags {
    type Args<'a> = ();
    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let raw = u16::read_options(reader, endian, ())?;
        Ok(Self::from_bytes(raw.to_le_bytes()))
    }
}

impl GeneralPurposeBitFlags {
    pub fn is_utf8(&self) -> bool {
        self.filename_utf8()
    }
    pub fn has_data_descriptor(&self) -> bool {
        self.data_descriptor()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, BinWrite)]
#[brw(repr = u16)]
pub enum CompressionMethod {
    None = 0,
    Shrunk = 1,
    Factor1 = 2,
    Factor2 = 3,
    Factor3 = 4,
    Factor4 = 5,
    Implode = 6,
    Deflate = 8,
    Deflate64 = 9,
    PKWARE = 10,
    BZIP2 = 12,
    LZMA = 14,
    CMPSC = 16,
    IBMTERSE = 18,
    LZ77 = 19,
    _ZSTD = 20,
    ZSTD = 93,
    MP3 = 94,
    XZ = 95,
    JPEG = 96,
    WavPack = 97,
    PPMd = 98,
    AeX = 99,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub struct DosTime {
    pub raw: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub struct DosDate {
    pub raw: u16,
}
