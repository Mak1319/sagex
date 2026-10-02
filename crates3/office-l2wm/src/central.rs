use std::io::{Read, Seek};

use binrw::{BinRead, BinResult, BinWrite};

use crate::extra::ExtraFields;
use crate::flags::{CompressionMethod, DosDate, DosTime, GeneralPurposeBitFlags};
use crate::local::LocalFileHeader;

#[derive(Debug, Clone)]
pub struct CentralDirectoryFileHeader {
    pub version_made: u16,
    pub version_extract: u16,
    pub flags: GeneralPurposeBitFlags,
    pub compression_method: CompressionMethod,
    pub mod_time: DosTime,
    pub mod_date: DosDate,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub file_name: Vec<u8>,
    pub extra: ExtraFields,
    pub comment: Vec<u8>,
    pub disk_number: u16,
    pub internal_attrs: u16,
    pub external_attrs: u32,
    pub local_header_offset: u32,
}

impl BinRead for CentralDirectoryFileHeader {
    type Args<'a> = ();
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let sig = u32::read_options(reader, endian, ())?;
        if sig != 0x02014B50 {
            return Err(binrw::Error::AssertFail {
                pos: reader.stream_position()?,
                message: "bad CDFH magic".into(),
            });
        }
        let version_made = u16::read_options(reader, endian, ())?;
        let version_extract = u16::read_options(reader, endian, ())?;
        let flags = GeneralPurposeBitFlags::read_options(reader, endian, ())?;
        let compression_method = CompressionMethod::read_options(reader, endian, ())?;
        let mod_time = DosTime::read_options(reader, endian, ())?;
        let mod_date = DosDate::read_options(reader, endian, ())?;
        let crc32 = u32::read_options(reader, endian, ())?;
        let compressed_size = u32::read_options(reader, endian, ())?;
        let uncompressed_size = u32::read_options(reader, endian, ())?;
        let fn_len = u16::read_options(reader, endian, ())?;
        let extra_len = u16::read_options(reader, endian, ())?;
        let comment_len = u16::read_options(reader, endian, ())?;
        let disk_number = u16::read_options(reader, endian, ())?;
        let internal_attrs = u16::read_options(reader, endian, ())?;
        let external_attrs = u32::read_options(reader, endian, ())?;
        let local_header_offset = u32::read_options(reader, endian, ())?;
        let mut file_name = vec![0u8; fn_len as usize];
        reader.read_exact(&mut file_name)?;
        let extra = ExtraFields::parse(reader, endian, extra_len)?;
        let mut comment = vec![0u8; comment_len as usize];
        reader.read_exact(&mut comment)?;
        Ok(Self {
            version_made, version_extract, flags, compression_method,
            mod_time, mod_date, crc32, compressed_size, uncompressed_size,
            file_name, extra, comment, disk_number, internal_attrs,
            external_attrs, local_header_offset,
        })
    }
}

impl BinWrite for CentralDirectoryFileHeader {
    type Args<'a> = ();
    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        0x02014B50u32.write_options(writer, endian, ())?;
        self.version_made.write_options(writer, endian, ())?;
        self.version_extract.write_options(writer, endian, ())?;
        self.flags.write_options(writer, endian, ())?;
        self.compression_method.write_options(writer, endian, ())?;
        self.mod_time.write_options(writer, endian, ())?;
        self.mod_date.write_options(writer, endian, ())?;
        self.crc32.write_options(writer, endian, ())?;
        self.compressed_size.write_options(writer, endian, ())?;
        self.uncompressed_size.write_options(writer, endian, ())?;
        (self.file_name.len() as u16).write_options(writer, endian, ())?;
        let mut ec = std::io::Cursor::new(Vec::new());
        self.extra.write_options(&mut ec, endian, ())?;
        let eb = ec.into_inner();
        (eb.len() as u16).write_options(writer, endian, ())?;
        (self.comment.len() as u16).write_options(writer, endian, ())?;
        self.disk_number.write_options(writer, endian, ())?;
        self.internal_attrs.write_options(writer, endian, ())?;
        self.external_attrs.write_options(writer, endian, ())?;
        self.local_header_offset.write_options(writer, endian, ())?;
        writer.write_all(&self.file_name)?;
        writer.write_all(&eb)?;
        writer.write_all(&self.comment)?;
        Ok(())
    }
}

impl CentralDirectoryFileHeader {
    pub fn local_header_offset_resolved(&self) -> u64 {
        if self.local_header_offset != 0xFFFF_FFFF {
            return self.local_header_offset as u64;
        }
        self.extra.local_offset_override().unwrap_or(0)
    }

    pub fn file_name_str(&self) -> String {
        String::from_utf8_lossy(&self.file_name).into_owned()
    }

    pub fn read_local(&self, data: &[u8]) -> BinResult<LocalFileHeader> {
        use std::io::{Cursor, SeekFrom};
        let mut c = Cursor::new(data);
        c.seek(SeekFrom::Start(self.local_header_offset_resolved()))?;
        LocalFileHeader::read_options(&mut c, binrw::Endian::Little, ())
    }
}
