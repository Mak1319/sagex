use std::io::{Read, Seek};

use binrw::{BinRead, BinResult, BinWrite};

use crate::extra::ExtraFields;
use crate::flags::{CompressionMethod, DosDate, DosTime, GeneralPurposeBitFlags};

#[derive(Debug, Clone)]
pub struct LocalFileHeader {
    pub version: u16,
    pub flags: GeneralPurposeBitFlags,
    pub compression_method: CompressionMethod,
    pub mod_time: DosTime,
    pub mod_date: DosDate,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub file_name: Vec<u8>,
    pub extra: ExtraFields,
    pub data: Vec<u8>,
}

impl BinRead for LocalFileHeader {
    type Args<'a> = ();
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let sig = u32::read_options(reader, endian, ())?;
        if sig != 0x04034B50 {
            return Err(binrw::Error::AssertFail {
                pos: reader.stream_position()?,
                message: "bad LFH magic".into(),
            });
        }
        let version = u16::read_options(reader, endian, ())?;
        let flags = GeneralPurposeBitFlags::read_options(reader, endian, ())?;
        let compression_method = CompressionMethod::read_options(reader, endian, ())?;
        let mod_time = DosTime::read_options(reader, endian, ())?;
        let mod_date = DosDate::read_options(reader, endian, ())?;
        let crc32 = u32::read_options(reader, endian, ())?;
        let compressed_size = u32::read_options(reader, endian, ())?;
        let uncompressed_size = u32::read_options(reader, endian, ())?;
        let fn_len = u16::read_options(reader, endian, ())?;
        let extra_len = u16::read_options(reader, endian, ())?;
        let mut file_name = vec![0u8; fn_len as usize];
        reader.read_exact(&mut file_name)?;
        let extra = ExtraFields::parse(reader, endian, extra_len)?;
        let probe = Self {
            version, flags, compression_method, mod_time, mod_date,
            crc32, compressed_size, uncompressed_size,
            file_name, extra, data: Vec::new(),
        };
        let n = probe.data_len() as usize;
        let mut data = vec![0u8; n];
        reader.read_exact(&mut data)?;
        Ok(Self { data, ..probe })
    }
}

impl BinWrite for LocalFileHeader {
    type Args<'a> = ();
    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        0x04034B50u32.write_options(writer, endian, ())?;
        self.version.write_options(writer, endian, ())?;
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
        writer.write_all(&self.file_name)?;
        writer.write_all(&eb)?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}

impl LocalFileHeader {
    pub fn data_len(&self) -> u64 {
        let base = if self.compression_method == CompressionMethod::None {
            self.uncompressed_size as u64
        } else {
            self.compressed_size as u64
        };
        if base != 0xFFFF_FFFF {
            return base;
        }
        self.extra
            .data_size_override(self.compression_method)
            .unwrap_or(0)
    }

    pub fn file_name_str(&self) -> String {
        String::from_utf8_lossy(&self.file_name).into_owned()
    }

    pub fn is_data_descriptor(&self) -> bool {
        self.flags.has_data_descriptor()
    }

    /// Raw stored bytes decoded per compression method.
    pub fn decoded_data(&self) -> crate::error::OL2WMResult<Vec<u8>> {
        use crate::error::Error;
        match self.compression_method {
            CompressionMethod::None => Ok(self.data.clone()),
            CompressionMethod::Deflate | CompressionMethod::Deflate64 => {
                use flate2::read::DeflateDecoder;
                use std::io::Read;
                let mut d = DeflateDecoder::new(&self.data[..]);
                let mut out = Vec::new();
                d.read_to_end(&mut out).map_err(|_| Error::Decompress {
                    part: self.file_name_str(),
                })?;
                Ok(out)
            }
            m => Err(Error::UnsupportedCompression { method: m as u16 }),
        }
    }

    /// Replace content, preserving the entry's compression method.
    pub fn set_decoded_data(&mut self, raw: Vec<u8>) -> crate::error::OL2WMResult<()> {
        use crate::error::Error;
        let raw_len = raw.len() as u32;
        let raw_crc = crate::watermark::crc32(&raw);
        let stored = match self.compression_method {
            CompressionMethod::None => raw,
            CompressionMethod::Deflate | CompressionMethod::Deflate64 => {
                use flate2::write::DeflateEncoder;
                use flate2::Compression;
                use std::io::Write;
                let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
                e.write_all(&raw).map_err(|_| Error::Decompress {
                    part: self.file_name_str(),
                })?;
                e.finish().map_err(|_| Error::Decompress {
                    part: self.file_name_str(),
                })?
            }
            m => return Err(Error::UnsupportedCompression { method: m as u16 }),
        };
        self.compressed_size = stored.len() as u32;
        self.uncompressed_size = raw_len;
        self.crc32 = raw_crc;
        self.data = stored;
        Ok(())
    }
}
