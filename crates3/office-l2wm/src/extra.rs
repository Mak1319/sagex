use std::io::{Read, Seek, SeekFrom};

use binrw::{BinRead, BinResult, BinWrite, Endian};

use crate::flags::CompressionMethod;

#[derive(Debug, Clone)]
pub struct ExtraField {
    pub tag: u16,
    pub size: u16,
    pub value: ExtraFieldValue,
}

impl BinRead for ExtraField {
    type Args<'a> = ();
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let tag = u16::read_options(reader, endian, ())?;
        let size = u16::read_options(reader, endian, ())?;
        let mut raw = vec![0u8; size as usize];
        reader.read_exact(&mut raw)?;
        let value = ExtraFieldValue::read_for(tag, size, raw)?;
        Ok(Self { tag, size, value })
    }
}

impl BinWrite for ExtraField {
    type Args<'a> = ();
    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        self.tag.write_options(writer, endian, ())?;
        let bytes = self.value.to_bytes();
        (bytes.len() as u16).write_options(writer, endian, ())?;
        writer.write_all(&bytes)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ExtraFieldValue {
    Zip64(Zip64SizeInfo),
    Ut(UtInfo),
    Ntfs(NtfsInfo),
    NewUnix(NewUnixInfo),
    InfoZip(InfoZipInfo),
    Unknown(Vec<u8>),
}

impl ExtraFieldValue {
    fn read_for(tag: u16, size: u16, data: Vec<u8>) -> BinResult<Self> {
        use std::io::Cursor;
        let mut c = Cursor::new(data);
        let v = match tag {
            0x0001 => Self::Zip64(Zip64SizeInfo::read_options(
                &mut c,
                Endian::Little,
                (size,),
            )?),
            0x5455 => Self::Ut(UtInfo::read_options(&mut c, Endian::Little, (size,))?),
            0x000a => Self::Ntfs(NtfsInfo::read_options(&mut c, Endian::Little, ())?),
            0x7875 => Self::NewUnix(NewUnixInfo::read_options(&mut c, Endian::Little, ())?),
            0x5855 => Self::InfoZip(InfoZipInfo::read_options(&mut c, Endian::Little, (size,))?),
            _ => Self::Unknown(c.into_inner()),
        };
        Ok(v)
    }

    fn to_bytes(&self) -> Vec<u8> {
        use std::io::Cursor;
        match self {
            Self::Unknown(v) => v.clone(),
            _ => {
                let mut c = Cursor::new(Vec::new());
                let _ = match self {
                    Self::Zip64(z) => z.write_options(&mut c, Endian::Little, ()),
                    Self::Ut(u) => u.write_options(&mut c, Endian::Little, ()),
                    Self::Ntfs(n) => n.write_options(&mut c, Endian::Little, ()),
                    Self::NewUnix(n) => n.write_options(&mut c, Endian::Little, ()),
                    Self::InfoZip(i) => i.write_options(&mut c, Endian::Little, ()),
                    Self::Unknown(_) => Ok(()),
                };
                c.into_inner()
            }
        }
    }
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(little, import(size: u16))]
#[bw(little)]
pub struct Zip64SizeInfo {
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    #[br(if(size > 16))]
    #[bw(if(local_header_offset.is_some()))]
    pub local_header_offset: Option<u64>,
    #[br(if(size > 24))]
    #[bw(if(disk_start_number.is_some()))]
    pub disk_start_number: Option<u32>,
}

#[derive(Debug, Clone, BinWrite)]
pub struct UtInfo {
    pub flags: u8,
    pub mod_time: Option<u32>,
    pub access_time: Option<u32>,
    pub creation_time: Option<u32>,
}

impl BinRead for UtInfo {
    type Args<'a> = (u16,);
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let flags = u8::read_options(reader, endian, ())?;
        let mut remaining = args.0.saturating_sub(1) as usize;
        let mut take = |reader: &mut R| -> BinResult<Option<u32>> {
            if remaining >= 4 {
                remaining -= 4;
                Ok(Some(u32::read_options(reader, endian, ())?))
            } else {
                Ok(None)
            }
        };
        let mod_time = if flags & 1 != 0 { take(reader)? } else { None };
        let access_time = if flags & 2 != 0 { take(reader)? } else { None };
        let creation_time = if flags & 4 != 0 { take(reader)? } else { None };
        Ok(Self {
            flags,
            mod_time,
            access_time,
            creation_time,
        })
    }
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
pub struct NtfsInfo {
    pub reserved: u32,
    pub tag: u16,
    pub t_size: u16,
    pub mod_time: u64,
    pub access_time: u64,
    pub creation_time: u64,
}

#[derive(Debug, Clone, BinWrite)]
pub struct NewUnixInfo {
    pub version: u8,
    pub uid: Vec<u8>,
    pub gid: Vec<u8>,
}

impl BinRead for NewUnixInfo {
    type Args<'a> = ();
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let version = u8::read_options(reader, endian, ())?;
        let uid_size = u8::read_options(reader, endian, ())?;
        let mut uid = vec![0u8; uid_size as usize];
        reader.read_exact(&mut uid)?;
        let gid_size = u8::read_options(reader, endian, ())?;
        let mut gid = vec![0u8; gid_size as usize];
        reader.read_exact(&mut gid)?;
        Ok(Self { version, uid, gid })
    }
}

#[derive(Debug, Clone, BinWrite)]
pub struct InfoZipInfo {
    pub access_time: u32,
    pub mod_time: u32,
    pub uid: Option<u16>,
    pub gid: Option<u16>,
}

impl BinRead for InfoZipInfo {
    type Args<'a> = (u16,);
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let access_time = u32::read_options(reader, endian, ())?;
        let mod_time = u32::read_options(reader, endian, ())?;
        let uid = if args.0 > 8 {
            Some(u16::read_options(reader, endian, ())?)
        } else {
            None
        };
        let gid = if args.0 > 10 {
            Some(u16::read_options(reader, endian, ())?)
        } else {
            None
        };
        Ok(Self {
            access_time,
            mod_time,
            uid,
            gid,
        })
    }
}

/// Owned extra-field list with query API.
#[derive(Debug, Clone, Default)]
pub struct ExtraFields(pub Vec<ExtraField>);

impl ExtraFields {
    pub fn parse<R: Read + Seek>(reader: &mut R, endian: Endian, byte_len: u16) -> BinResult<Self> {
        let start = reader.stream_position()?;
        let end = start + byte_len as u64;
        let mut out = Vec::new();
        while Self::has_more(reader, end)? {
            out.push(ExtraField::read_options(reader, endian, ())?);
        }
        let pos = reader.stream_position()?;
        if pos < end {
            reader.seek(SeekFrom::Start(end))?;
        }
        Ok(Self(out))
    }

    fn has_more<R: Read + Seek>(reader: &mut R, end: u64) -> BinResult<bool> {
        let pos = reader.stream_position()?;
        if pos + 4 > end {
            return Ok(false);
        }
        let mut hdr = [0u8; 4];
        reader.read_exact(&mut hdr)?;
        reader.seek(SeekFrom::Start(pos))?;
        let tag = u16::from_le_bytes([hdr[0], hdr[1]]);
        let len = u16::from_le_bytes([hdr[2], hdr[3]]);
        Ok(!(tag == 0 || len == 0) && pos + 4 + len as u64 <= end)
    }

    pub fn zip64(&self) -> Option<&Zip64SizeInfo> {
        self.0.iter().find_map(|f| match &f.value {
            ExtraFieldValue::Zip64(z) => Some(z),
            _ => None,
        })
    }

    pub fn local_offset_override(&self) -> Option<u64> {
        self.zip64()?.local_header_offset
    }

    pub fn data_size_override(&self, method: CompressionMethod) -> Option<u64> {
        let z = self.zip64()?;
        Some(if method == CompressionMethod::None {
            z.uncompressed_size
        } else {
            z.compressed_size
        })
    }
}

impl BinRead for ExtraFields {
    type Args<'a> = (u16,);
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        Self::parse(reader, endian, args.0)
    }
}

impl BinWrite for ExtraFields {
    type Args<'a> = ();
    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        for f in &self.0 {
            f.write_options(writer, endian, ())?;
        }
        Ok(())
    }
}
