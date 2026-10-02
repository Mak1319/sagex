use std::io::{Read, Seek};

use binrw::{BinRead, BinResult, BinWrite};

use crate::central::CentralDirectoryFileHeader;

#[derive(Debug, Clone)]
pub struct Eocd64Locator {
    pub cdr_disk: u32,
    pub eocd_offset: u64,
    pub total_disks: u32,
}

impl BinRead for Eocd64Locator {
    type Args<'a> = ();
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let sig = u32::read_options(reader, endian, ())?;
        if sig != 0x07064B50 {
            return Err(binrw::Error::AssertFail {
                pos: reader.stream_position()?,
                message: "bad EOCD64 locator magic".into(),
            });
        }
        Ok(Self {
            cdr_disk: u32::read_options(reader, endian, ())?,
            eocd_offset: u64::read_options(reader, endian, ())?,
            total_disks: u32::read_options(reader, endian, ())?,
        })
    }
}

impl BinWrite for Eocd64Locator {
    type Args<'a> = ();
    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        0x07064B50u32.write_options(writer, endian, ())?;
        self.cdr_disk.write_options(writer, endian, ())?;
        self.eocd_offset.write_options(writer, endian, ())?;
        self.total_disks.write_options(writer, endian, ())?;
        Ok(())
    }
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(little)]
pub struct Eocd32 {
    pub disk_num: u16,
    pub disk_start: u16,
    pub cdr_count: u16,
    pub central_directory_record_count: u16,
    pub cd_size: u32,
    pub cd_offset: u32,
    pub comment_length: u16,
    #[br(count = comment_length)]
    pub comment: Vec<u8>,
}

impl Eocd32 {
    pub fn is_zip64_placeholder(&self) -> bool {
        self.cd_offset == 0xFFFF_FFFF || self.central_directory_record_count == 0xFFFF
    }
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(little)]
pub struct Eocd64 {
    pub eocd_size: u64,
    pub made_by_version: u16,
    pub version_needed: u16,
    pub disk_num: u32,
    pub disk_start: u32,
    pub cdr_count: u64,
    pub central_directory_record_count: u64,
    pub cd_size: u64,
    pub cd_offset: u64,
    #[br(count = eocd_size.saturating_sub(44))]
    pub extra: Vec<u8>,
    pub locator: Eocd64Locator,
    pub eocd32: [u8; 20],
    pub comment_length: u16,
    #[br(count = comment_length)]
    pub comment: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum Eocd {
    V32(Eocd32),
    V64(Eocd64),
}

impl Eocd {
    pub fn central_offset(&self) -> u64 {
        match self {
            Self::V32(e) => e.cd_offset as u64,
            Self::V64(e) => e.cd_offset,
        }
    }
    pub fn record_count(&self) -> u64 {
        match self {
            Self::V32(e) => e.central_directory_record_count as u64,
            Self::V64(e) => e.central_directory_record_count,
        }
    }
}

/// Scans for EOCD; owns the find_eocd algorithm.
pub struct EocdFinder<'a> {
    data: &'a [u8],
}

impl<'a> EocdFinder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn locate(&self) -> crate::error::OL2WMResult<(u64, Eocd)> {
        use crate::error::Error::EOCDNotFound;
        let n = self.data.len() as u64;
        let search_from = n.saturating_sub(65536);
        // naive reverse scan for PK\x05\x06
        let mut pos = search_from;
        let mut candidates: Vec<u64> = Vec::new();
        while pos + 4 <= n {
            if self.data[pos as usize..][..4] == [0x50, 0x4B, 0x05, 0x06] {
                candidates.push(pos);
            }
            pos += 1;
        }
        for off in candidates.into_iter().rev() {
            if let Some(hit) = self.try_at(off) {
                return Ok((off, hit));
            }
        }
        Err(EOCDNotFound)
    }

    fn try_at(&self, off: u64) -> Option<Eocd> {
        use std::io::Cursor;
        let mut c = Cursor::new(self.data);
        c.set_position(off + 4);
        // peek CDOffset/record count from EOCD32 layout
        let probe = |c: &mut Cursor<&[u8]>| -> Option<Eocd> {
            let e32 = Eocd32::read_options(c, binrw::Endian::Little, ()).ok()?;
            if e32.is_zip64_placeholder() {
                // locator 20 bytes before eocd
                if off < 20 {
                    return None;
                }
                let mut lc = Cursor::new(self.data);
                lc.set_position(off - 20);
                let loc = Eocd64Locator::read_options(&mut lc, binrw::Endian::Little, ()).ok()?;
                let mut ec = Cursor::new(self.data);
                // EOCD64 record starts with magic u32 then body
                ec.set_position(loc.eocd_offset);
                let magic = u32::read_options(&mut ec, binrw::Endian::Little, ()).ok()?;
                if magic != 0x06064B50 {
                    return None;
                }
                let e64 = Eocd64::read_options(&mut ec, binrw::Endian::Little, ()).ok()?;
                if !self.valid_central(e64.cd_offset) {
                    return None;
                }
                Some(Eocd::V64(e64))
            } else {
                if !self.valid_central(e32.cd_offset as u64) {
                    return None;
                }
                Some(Eocd::V32(e32))
            }
        };
        probe(&mut c)
    }

    fn valid_central(&self, cd_offset: u64) -> bool {
        let o = cd_offset as usize;
        self.data.len() >= o + 4 && self.data[o..o + 4] == [0x50, 0x4B, 0x01, 0x02]
    }

    pub fn read_central(&self, eocd: &Eocd) -> BinResult<Vec<CentralDirectoryFileHeader>> {
        use std::io::{Cursor, SeekFrom};
        let mut c = Cursor::new(self.data);
        c.seek(SeekFrom::Start(eocd.central_offset()))?;
        let count = eocd.record_count().min(10_000) as usize;
        let mut out = Vec::with_capacity(count.min(256));
        for _ in 0..count {
            match CentralDirectoryFileHeader::read_options(&mut c, binrw::Endian::Little, ()) {
                Ok(h) => out.push(h),
                Err(_) => break,
            }
        }
        Ok(out)
    }
}
