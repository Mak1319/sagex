use std::io::{Cursor, Seek, SeekFrom};

use binrw::{BinRead, Endian};

use crate::central::CentralDirectoryFileHeader;
use crate::eocd::{Eocd, EocdFinder};
use crate::error::{Error, OL2WMResult};
use crate::local::LocalFileHeader;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TreeNode {
    pub name: String,
    pub dtype: String,
    pub offset: u64,
    pub size: u64,
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn leaf(
        name: impl Into<String>,
        dtype: impl Into<String>,
        offset: u64,
        size: u64,
        value: impl Into<String>,
        comment: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            dtype: dtype.into(),
            offset,
            size,
            value: value.into(),
            comment: comment.into(),
            children: Vec::new(),
        }
    }

    pub fn branch(
        name: impl Into<String>,
        dtype: impl Into<String>,
        offset: u64,
        children: Vec<TreeNode>,
    ) -> Self {
        // spanned size: end-start where measurable, else 0
        let size = match (children.first(), children.last()) {
            (Some(f), Some(l)) => (l.offset + l.size).saturating_sub(f.offset),
            _ => 0,
        };
        Self {
            name: name.into(),
            dtype: dtype.into(),
            offset,
            size,
            value: String::new(),
            comment: String::new(),
            children,
        }
    }

    pub fn print_text(&self, indent: usize) {
        let pad = "  ".repeat(indent);
        let c = if self.comment.is_empty() { String::new() } else { format!("  // {}", self.comment) };
        if self.children.is_empty() {
            println!(
                "{pad}{} @ {} ({}, size={}) = {}{c}",
                self.name, self.offset, self.dtype, self.size, self.value
            );
        } else {
            println!("{pad}{} @ {} ({}, size={}) {}{c}", self.name, self.offset, self.dtype, self.size, self.value);
            for ch in &self.children {
                ch.print_text(indent + 1);
            }
        }
    }
}

fn lv(n: &str, d: &str, o: u64, s: u64, v: String, c: &str) -> TreeNode {
    TreeNode::leaf(n, d, o, s, v, c)
}

pub struct TreeBuilder<'a> {
    data: &'a [u8],
}

impl<'a> TreeBuilder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn build(&self) -> OL2WMResult<TreeNode> {
        let finder = EocdFinder::new(self.data);
        let (eocd_off, eocd) = finder.locate()?;
        let eocd_node = self.eocd_tree(&eocd, eocd_off)?;
        let central_off = eocd.central_offset();
        let count = eocd.record_count().min(10_000);
        let mut c = Cursor::new(self.data);
        c.seek(SeekFrom::Start(central_off)).map_err(|_| Error::EOCDNotFound)?;
        let mut file_nodes = Vec::new();
        for i in 0..count {
            let off = c.stream_position().map_err(|_| Error::EOCDNotFound)?;
            match CentralDirectoryFileHeader::read_options(&mut c, Endian::Little, ()) {
                Ok(h) => {
                    let end = c.stream_position().map_err(|_| Error::EOCDNotFound)?;
                    file_nodes.push(self.cdfh_tree(i, off, end, &h)?);
                }
                Err(_) => break,
            }
        }
        let files = TreeNode::branch("files", "struct CentralDirectoryFileHeader[]", central_off, file_nodes);
        let children = vec![eocd_node, files];
        let mut root = TreeNode::branch("archive", "struct ZipArchive", 0, children);
        root.size = self.data.len() as u64;
        Ok(root)
    }

    fn eocd_tree(&self, eocd: &Eocd, off: u64) -> OL2WMResult<TreeNode> {
        match eocd {
            Eocd::V32(e) => {
                let kids = vec![
                    lv("magic", "u32", off, 4, "0x06054B50".into(), "End of central directory signature"),
                    lv("disk_num", "u16", off + 4, 2, e.disk_num.to_string(), "Number of this disk"),
                    lv("disk_start", "u16", off + 6, 2, e.disk_start.to_string(), "Disk where central directory starts"),
                    lv("cdr_count", "u16", off + 8, 2, e.cdr_count.to_string(), "Number of central directory records on this disk"),
                    lv("record_count", "u16", off + 10, 2, e.central_directory_record_count.to_string(), "Total number of entries in the central directory"),
                    lv("cd_size", "u32", off + 12, 4, e.cd_size.to_string(), "Size of central directory (bytes)"),
                    lv("cd_offset", "u32", off + 16, 4, e.cd_offset.to_string(), "Offset of start of central directory, relative to start of archive"),
                    lv("comment_length", "u16", off + 20, 2, e.comment_length.to_string(), ""),
                    lv("comment", &format!("bytes[{}]", e.comment.len()), off + 22, e.comment.len() as u64, String::from_utf8_lossy(&e.comment).into_owned(), ""),
                ];
                Ok(TreeNode::branch("eocd", "struct EndOfCentralDirectory", off, kids))
            }
            Eocd::V64(e) => {
                let kids = vec![
                    lv("magic", "u32", off, 4, "0x06064B50".into(), "End of central directory signature"),
                    lv("eocd_size", "u64", off + 4, 8, e.eocd_size.to_string(), "Size of fixed fields + size of variable data - 12"),
                    lv("made_by_version", "u16", off + 12, 2, e.made_by_version.to_string(), "The version of zip this was authored by"),
                    lv("version_needed", "u16", off + 14, 2, e.version_needed.to_string(), "The minimum supported ZIP version needed to extract the file"),
                    lv("disk_num", "u32", off + 16, 4, e.disk_num.to_string(), "number of this disk"),
                    lv("disk_start", "u32", off + 20, 4, e.disk_start.to_string(), "Disk where central directory starts"),
                    lv("cdr_count", "u64", off + 24, 8, e.cdr_count.to_string(), "Number of central directory records on this disk"),
                    lv("record_count", "u64", off + 32, 8, e.central_directory_record_count.to_string(), "Total number of entries in the central directory"),
                    lv("cd_size", "u64", off + 40, 8, e.cd_size.to_string(), "Size of central directory (bytes)"),
                    lv("cd_offset", "u64", off + 48, 8, e.cd_offset.to_string(), "Offset of start of central directory, relative to start of archive"),
                ];
                Ok(TreeNode::branch("eocd", "struct EndOfCentralDirectory", off, kids))
            }
        }
    }

    fn cdfh_tree(&self, i: u64, off: u64, end: u64, h: &CentralDirectoryFileHeader) -> OL2WMResult<TreeNode> {
        // fixed header is 46 bytes; walk offsets exactly
        let mut o = off;
        let mut kids = Vec::new();
        let mut push = |name: &str, dtype: &str, size: u64, value: String, comment: &str| {
            kids.push(lv(name, dtype, o, size, value, comment));
            o += size;
        };
        push("magic", "u32", 4, "0x02014B50".into(), "Central directory file header signature");
        push("version_made", "u16", 2, h.version_made.to_string(), "Version file made by");
        push("version_extract", "u16", 2, h.version_extract.to_string(), "Minimum version needed to extract");
        push("flags", "bitfield GeneralPurposeBitFlags", 2, format!("{:?}", h.flags), "General purpose bit flag");
        push("compression", "enum CompressionMethod", 2, format!("{:?} ({})", h.compression_method, h.compression_method as u16), "Compression method");
        push("mod_time", "u16 (DOSTime)", 2, h.mod_time.raw.to_string(), "File last modification time");
        push("mod_date", "u16 (DOSDate)", 2, h.mod_date.raw.to_string(), "File last modification date");
        push("crc32", "u32", 4, format!("{:08x} ({})", h.crc32, h.crc32), "CRC-32 of uncompressed data");
        push("compressed_size", "u32", 4, h.compressed_size.to_string(), "Compressed size");
        push("uncompressed_size", "u32", 4, h.uncompressed_size.to_string(), "Uncompressed size");
        push("file_name_length", "u16", 2, h.file_name.len().to_string(), "File name length (n)");
        push("extra_length", "u16", 2, h.extra.0.iter().map(|f| f.size as usize + 4).sum::<usize>().to_string(), "Extra field length (m)");
        push("comment_length", "u16", 2, h.comment.len().to_string(), "File comment length");
        push("disk_number", "u16", 2, h.disk_number.to_string(), "Disk number where file starts");
        push("internal_attrs", "u16", 2, h.internal_attrs.to_string(), "Internal file attributes");
        push("external_attrs", "u32", 4, h.external_attrs.to_string(), "External file attributes");
        push("local_header_offset", "u32", 4, format!("{} (resolved {})", h.local_header_offset, h.local_header_offset_resolved()), "Relative offset of local header");
        kids.push(lv("file_name", &format!("bytes[{}]", h.file_name.len()), o, h.file_name.len() as u64, h.file_name_str(), "File name"));
        o += h.file_name.len() as u64;
        for (j, f) in h.extra.0.iter().enumerate() {
            kids.push(lv(&format!("extra[{j}]"), "struct ExtraField", o, f.size as u64 + 4, format!("tag=0x{:04x} size={}", f.tag, f.size), extra_comment(f.tag)));
            o += f.size as u64 + 4;
        }
        if !h.comment.is_empty() {
            kids.push(lv("comment", &format!("bytes[{}]", h.comment.len()), o, h.comment.len() as u64, String::from_utf8_lossy(&h.comment).into_owned(), "File comment"));
            o += h.comment.len() as u64;
        }
        let _ = end;
        let lfh_off = h.local_header_offset_resolved();
        kids.push(self.lfh_tree(lfh_off)?);
        let _ = o;
        Ok(TreeNode::branch(format!("CDFH[{i}]"), "struct CentralDirectoryFileHeader", off, kids))
    }

    fn lfh_tree(&self, off: u64) -> OL2WMResult<TreeNode> {
        let mut c = Cursor::new(self.data);
        c.seek(SeekFrom::Start(off)).map_err(|_| Error::EOCDNotFound)?;
        let h = LocalFileHeader::read_options(&mut c, Endian::Little, ()).map_err(|_| Error::EOCDNotFound)?;
        let end = c.stream_position().map_err(|_| Error::EOCDNotFound)?;
        let data_len = h.data_len();
        let data_off = end - data_len;
        let mut o = off;
        let mut kids = Vec::new();
        let mut push = |name: &str, dtype: &str, size: u64, value: String, comment: &str| {
            kids.push(lv(name, dtype, o, size, value, comment));
            o += size;
        };
        push("magic", "u32", 4, "0x04034B50".into(), "Local file header signature");
        push("version", "u16", 2, h.version.to_string(), "The minimum supported ZIP specification version needed to extract the file");
        push("flags", "bitfield GeneralPurposeBitFlags", 2, format!("{:?}", h.flags), "General purpose bit flag");
        push("compression", "enum CompressionMethod", 2, format!("{:?}", h.compression_method), "Compression method");
        push("mod_time", "u16 (DOSTime)", 2, h.mod_time.raw.to_string(), "File last modification time");
        push("mod_date", "u16 (DOSDate)", 2, h.mod_date.raw.to_string(), "File last modification date");
        push("crc32", "u32", 4, format!("{:08x}", h.crc32), "CRC-32");
        push("compressed_size", "u32", 4, h.compressed_size.to_string(), "Compressed size");
        push("uncompressed_size", "u32", 4, h.uncompressed_size.to_string(), "Uncompressed size");
        push("file_name_length", "u16", 2, h.file_name.len().to_string(), "File name length (n)");
        let extra_bytes: usize = h.extra.0.iter().map(|f| f.size as usize + 4).sum();
        push("extra_length", "u16", 2, extra_bytes.to_string(), "Extra field length (m)");
        kids.push(lv("file_name", &format!("bytes[{}]", h.file_name.len()), o, h.file_name.len() as u64, h.file_name_str(), "File Name"));
        o += h.file_name.len() as u64;
        for (j, f) in h.extra.0.iter().enumerate() {
            kids.push(lv(&format!("extra[{j}]"), "struct ExtraField", o, f.size as u64 + 4, format!("tag=0x{:04x}", f.tag), extra_comment(f.tag)));
            o += f.size as u64 + 4;
        }
        kids.push(lv("data", &format!("bytes[{data_len}]"), data_off, data_len, format!("<skipped {data_len} bytes>"), "File data (raw bytes omitted)"));
        Ok(TreeNode::branch("LFH", "struct LocalFileHeader", off, kids))
    }
}

fn extra_comment(tag: u16) -> &'static str {
    match tag {
        0x0001 => "Zip64 extended information",
        0x5455 => "Extended timestamp",
        0x000a => "NTFS extra field",
        0x7875 => "New Unix extra field",
        0x5855 => "Info-ZIP Unix extra field",
        _ => "Extra field",
    }
}
