use std::io::Cursor;

use binrw::{BinResult, BinWrite, Endian};

use crate::central::CentralDirectoryFileHeader;
use crate::eocd::{Eocd, EocdFinder};
use crate::error::{Error, OL2WMResult};
use crate::local::LocalFileHeader;
use crate::watermark::FontWatermark;

/// Top-level facade owning EOCD + central directory.
pub struct ZipArchive {
    eocd_offset: u64,
    eocd: Eocd,
    files: Vec<CentralDirectoryFileHeader>,
}

impl ZipArchive {
    pub fn open(data: &[u8]) -> crate::error::OL2WMResult<Self> {
        let finder = EocdFinder::new(data);
        let (eocd_offset, eocd) = finder.locate()?;
        let files = finder
            .read_central(&eocd)
            .map_err(|_| crate::error::Error::EOCDNotFound)?;
        Ok(Self { eocd_offset, eocd, files })
    }

    pub fn eocd(&self) -> &Eocd {
        &self.eocd
    }
    pub fn eocd_offset(&self) -> u64 {
        self.eocd_offset
    }
    pub fn files(&self) -> &[CentralDirectoryFileHeader] {
        &self.files
    }

    pub fn get(&self, name: &str) -> Option<&CentralDirectoryFileHeader> {
        self.files.iter().find(|f| f.file_name_str() == name)
    }

    pub fn container_kind(&self) -> crate::watermark::ContainerKind {
        crate::watermark::ContainerKind::detect(
            &self.files.iter().map(|f| f.file_name_str()).collect::<Vec<_>>(),
        )
    }

    pub fn read_local(&self, index: usize, data: &[u8]) -> BinResult<LocalFileHeader> {
        self.files[index].read_local(data)
    }

    fn collect_locals(&self, data: &[u8]) -> OL2WMResult<Vec<LocalFileHeader>> {
        let mut out = Vec::new();
        for f in &self.files {
            out.push(f.read_local(data).map_err(|_| Error::EOCDNotFound)?);
        }
        Ok(out)
    }

    fn random_gaps(&self, n_old: usize, n_new: usize) -> Vec<usize> {
        // time-hashed xorshift; gaps in 0..=n_old
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ (d.as_secs() << 32))
            .unwrap_or(0x9e3779b9);
        let mut s = t | 1;
        let mut gaps = Vec::new();
        for _ in 0..n_new {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            gaps.push((s % (n_old as u64 + 1)) as usize);
        }
        gaps
    }

    fn write_eocd32(
        cur: &mut Cursor<Vec<u8>>,
        count: u16,
        cd_size: u32,
        cd_offset: u32,
    ) -> OL2WMResult<()> {
        0x06054B50u32.write_options(cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        0u16.write_options(cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        0u16.write_options(cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        count.write_options(cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        count.write_options(cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        cd_size.write_options(cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        cd_offset.write_options(cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        0u16.write_options(cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        Ok(())
    }

    fn splice_before(
        locals: &mut [LocalFileHeader],
        part: &str,
        closing: &[u8],
        insert: &str,
    ) -> OL2WMResult<()> {
        let idx = locals
            .iter()
            .position(|l| l.file_name_str() == part)
            .ok_or(Error::MissingPart { name: part.into() })?;
        let raw = locals[idx].decoded_data()?;
        let patched = Self::insert_xml_before(&raw, closing, insert)?;
        locals[idx].set_decoded_data(patched)?;
        Ok(())
    }

    /// Raw-string insert before a closing tag.
    fn insert_xml_before(xml: &[u8], closing: &[u8], insert: &str) -> OL2WMResult<Vec<u8>> {
        let pos = xml
            .windows(closing.len())
            .position(|w| w == closing)
            .ok_or(Error::Xml { stage: "parent-end-not-found" })?;
        let mut out = Vec::with_capacity(xml.len() + insert.len());
        out.extend_from_slice(&xml[..pos]);
        out.extend_from_slice(insert.as_bytes());
        out.extend_from_slice(&xml[pos..]);
        Ok(out)
    }

    /// Font watermark: embed odttf + invisible run + fontTable + links.
    /// Text is camouflage (invisible markers); the payload lives in glyph outlines.
    pub fn embed_font_watermark(
        &self,
        data: &[u8],
        marks: &[FontWatermark],
    ) -> OL2WMResult<Vec<u8>> {
        use crate::watermark::{ContainerKind, CUSTOM_REL_TYPE, FONT_REL_TYPE};
        let mut locals = self.collect_locals(data)?;
        let names: Vec<String> = locals.iter().map(|l| l.file_name_str()).collect();
        let kind = ContainerKind::detect(&names);
        let (fonttable, main_rels, text_part, text_closing, use_header) = match kind {
            ContainerKind::OpcDocx => {
                let hdr = names.iter().find(|n| n.starts_with("word/header")).cloned();
                (
                    "word/fontTable.xml".to_string(),
                    "word/_rels/document.xml.rels".to_string(),
                    hdr.unwrap_or_else(|| "word/document.xml".to_string()),
                    "</w:body>".as_bytes(),
                    true,
                )
            }
            ContainerKind::OpcXlsx => (
                "xl/fontTable.xml".to_string(),
                "xl/_rels/workbook.xml.rels".to_string(),
                "xl/workbook.xml".to_string(),
                "</workbook>".as_bytes(),
                false,
            ),
            ContainerKind::OpcPptx => (
                "ppt/fontTable.xml".to_string(),
                "ppt/_rels/presentation.xml.rels".to_string(),
                String::new(),
                &[][..],
                false,
            ),
            _ => {
                return Err(Error::UnsupportedContainer {
                    kind: "font channel needs docx, xlsx or pptx",
                });
            }
        };
        let _ = use_header;
        // 1. content types: font override (+ fontTable override if created)
        let ct_idx = names
            .iter()
            .position(|n| n == "[Content_Types].xml")
            .ok_or(Error::MissingPart { name: "[Content_Types].xml".into() })?;
        let mut ct = locals[ct_idx].decoded_data()?;
        for m in marks {
            let el = format!(
                "<Override PartName=\"/{}\" ContentType=\"application/vnd.openxmlformats-officedocument.obfuscatedFont\"/>",
                m.path
            );
            if !ct.windows(m.path.len()).any(|w| w == m.path.as_bytes()) {
                ct = Self::insert_xml_before(&ct, b"</Types>", &el)?;
            }
        }
        let fonttable_created = !names.iter().any(|n| n == &fonttable);
        if fonttable_created {
            let el = format!(
                "<Override PartName=\"/{fonttable}\" ContentType=\"application/vnd.openxmlformats-officedocument.fontTable+xml\"/>"
            );
            ct = Self::insert_xml_before(&ct, b"</Types>", &el)?;
        }
        // xlsx veryHidden sheet needs its override now (created below)
        let xlsx_sheet = kind == ContainerKind::OpcXlsx;
        if xlsx_sheet {
            let el = "<Override PartName=\"/xl/worksheets/sheetWm.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>";
            if !ct.windows(20).any(|w| w == b"sheetWm.xml") {
                ct = Self::insert_xml_before(&ct, b"</Types>", el)?;
            }
        }
        locals[ct_idx].set_decoded_data(ct)?;
        // 2. fontTable.xml create or extend
        if fonttable_created {
            let mut ft = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:fonts xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">");
            for m in marks {
                ft.push_str(&format!(
                    "<w:font w:name=\"{}\"><w:charset w:val=\"00\"/><w:family w:val=\"auto\"/><w:pitch w:val=\"variable\"/><w:embedRegular w:fontKey=\"{}\" w:uri=\"{}\"/></w:font>",
                    m.face,
                    m.font_key,
                    m.path.split('/').last().unwrap_or(&m.path)
                ));
            }
            ft.push_str("</w:fonts>");
            let mut nl = marks[0].to_local();
            nl.file_name = fonttable.as_bytes().to_vec();
            nl.compressed_size = 0;
            nl.uncompressed_size = 0;
            nl.crc32 = 0;
            nl.data = Vec::new();
            nl.set_decoded_data(ft.into_bytes())?;
            locals.push(nl);
        } else {
            let idx = locals.iter().position(|l| l.file_name_str() == fonttable).unwrap();
            let mut raw = locals[idx].decoded_data()?;
            for m in marks {
                if !raw.windows(m.face.len()).any(|w| w == m.face.as_bytes()) {
                    let el = format!(
                        "<w:font w:name=\"{}\"><w:embedRegular w:fontKey=\"{}\" w:uri=\"{}\"/></w:font>",
                        m.face,
                        m.font_key,
                        m.path.split('/').last().unwrap_or(&m.path)
                    );
                    raw = Self::insert_xml_before(
                        &raw, b"</w:fonts>", &el,
                    )?;
                }
            }
            locals[idx].set_decoded_data(raw)?;
        }
        // 3. rels: both rel kinds targeting the font file
        let ridx = locals
            .iter()
            .position(|l| l.file_name_str() == main_rels)
            .ok_or(Error::MissingPart { name: main_rels.clone() })?;
        let mut rx = locals[ridx].decoded_data()?;
        for m in marks {
            let media_t = FONT_REL_TYPE;
            let target = format!("/{}", m.path);
            for (id, ty) in [(m.media_rel_id(), media_t), (m.custom_rel_id(), CUSTOM_REL_TYPE)] {
                if !rx.windows(id.len()).any(|w| w == id.as_bytes()) {
                    let el = format!("<Relationship Id=\"{id}\" Type=\"{ty}\" Target=\"{target}\"/>");
                    rx = Self::insert_xml_before(
                        &rx, b"</Relationships>", &el,
                    )?;
                }
            }
        }
        locals[ridx].set_decoded_data(rx)?;
        // 4. invisible marker text using the font
        match kind {
            ContainerKind::OpcDocx => {
                let marker: String = marks
                    .iter()
                    .map(|m| {
                        format!(
                            "<w:p><w:r><w:rPr><w:rFonts w:ascii=\"{0}\" w:hAnsi=\"{0}\" w:cs=\"{0}\"/><w:vanish/><w:sz w:val=\"2\"/><w:color w:val=\"FFFFFF\"/></w:rPr><w:t xml:space=\"preserve\">\u{200b}\u{200c}\u{200d}\u{2060}</w:t></w:r></w:p>",
                            m.face
                        )
                    })
                    .collect();
                let closing = if text_part.ends_with(".xml") && text_part.contains("header") {
                    "</w:hdr>"
                } else {
                    "</w:body>"
                };
                Self::splice_before(&mut locals, &text_part, closing.as_bytes(), &marker)?;
            }
            ContainerKind::OpcXlsx => {
                // veryHidden sheet with 1pt white run in our face
                let sidx = locals
                    .iter()
                    .position(|l| l.file_name_str() == "xl/styles.xml")
                    .ok_or(Error::MissingPart { name: "xl/styles.xml".into() })?;
                let mut st = locals[sidx].decoded_data()?;
                for m in marks {
                    if !st.windows(m.face.len()).any(|w| w == m.face.as_bytes()) {
                        let fnt = format!(
                            "<font><sz val=\"1\"/><color rgb=\"FFFFFFFF\"/><name val=\"{}\"/></font>",
                            m.face
                        );
                        st = Self::insert_xml_before(
                            &st, b"</fonts>", &fnt,
                        )?;
                    }
                }
                // count fonts -> xf index = fonts-1 ... simpler: count <font> occurrences
                let nfonts = st.windows(6).filter(|w| *w == b"<font>").count() as u32;
                // add cellXfs (append, index = count); count xfs first
                let nxf = st.windows(7).filter(|w| *w == b"<xf ").count() as u32;
                let xf_id = nxf; // new xf appended at end
                let xf = format!("<xf numFmtId=\"0\" fontId=\"{}\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>", nfonts - 1);
                st = Self::insert_xml_before(&st, b"</cellXfs>", &xf)?;
                locals[sidx].set_decoded_data(st)?;
                // sheet file
                let sheet = format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" s=\"{xf_id}\" t=\"inlineStr\"><is><t>&#x200B;&#x200C;&#x200D;&#x2060;</t></is></c></row></sheetData></worksheet>"
                );
                let mut nl = marks[0].to_local();
                nl.file_name = b"xl/worksheets/sheetWm.xml".to_vec();
                nl.data = Vec::new();
                nl.compressed_size = 0;
                nl.uncompressed_size = 0;
                nl.crc32 = 0;
                nl.set_decoded_data(sheet.into_bytes())?;
                locals.push(nl);
                // workbook.xml: veryHidden sheet entry
                let wb_idx = locals
                    .iter()
                    .position(|l| l.file_name_str() == "xl/workbook.xml")
                    .ok_or(Error::MissingPart { name: "xl/workbook.xml".into() })?;
                let mut wb = locals[wb_idx].decoded_data()?;
                if !wb.windows(9).any(|w| w == b"sheetWm\"") {
                    // new sheet id = max existing + 1; find rId not used: rIdWm
                    let el = "<sheet name=\"wm\" sheetId=\"999\" state=\"veryHidden\" r:id=\"rIdWm\"/>";
                    wb = Self::insert_xml_before(&wb, b"</sheets>", el)?;
                }
                locals[wb_idx].set_decoded_data(wb)?;
                // workbook rels: sheetWm target
                let wr_idx = locals
                    .iter()
                    .position(|l| l.file_name_str() == "xl/_rels/workbook.xml.rels")
                    .ok_or(Error::MissingPart { name: "xl/_rels/workbook.xml.rels".into() })?;
                let mut wr = locals[wr_idx].decoded_data()?;
                if !wr.windows(6).any(|w| w == b"rIdWm\"") {
                    let el = "<Relationship Id=\"rIdWm\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheetWm.xml\"/>";
                    wr = Self::insert_xml_before(
                        &wr, b"</Relationships>", el,
                    )?;
                }
                locals[wr_idx].set_decoded_data(wr)?;
                let _ = text_closing;
            }
            ContainerKind::OpcPptx => {
                // corner white 1pt textbox on first slide
                let slide = names
                    .iter()
                    .find(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
                    .cloned()
                    .ok_or(Error::MissingPart { name: "ppt/slides/slide?.xml".into() })?;
                let boxes: String = marks
                    .iter()
                    .map(|m| {
                        format!(
                            "<p:sp><p:nvSpPr><p:cNvPr id=\"9999\" name=\"wm\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"12700\" cy=\"12700\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill></p:spPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr sz=\"100\"><a:latin typeface=\"{0}\"/><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill></a:rPr><a:t>&#x200B;&#x200C;&#x200D;&#x2060;</a:t></a:r></a:p></p:txBody></p:sp>",
                            m.face
                        )
                    })
                    .collect();
                Self::splice_before(&mut locals, &slide, b"</p:spTree>", &boxes)?;
            }
            _ => {}
        }
        // 5. splice font files at random gaps + serialize everything
        let new_locals: Vec<LocalFileHeader> = marks.iter().map(|m| m.to_local()).collect();
        let gaps = self.random_gaps(locals.len(), new_locals.len());
        let n_old = locals.len();
        let mut slots: Vec<Vec<LocalFileHeader>> = vec![Vec::new(); n_old + 1];
        for (j, nl) in new_locals.into_iter().enumerate() {
            slots[gaps[j] % (n_old + 1)].push(nl);
        }
        let mut ordered: Vec<(bool, LocalFileHeader)> = Vec::new();
        for (i, l) in locals.into_iter().enumerate() {
            for nl in slots[i].drain(..) {
                ordered.push((true, nl));
            }
            ordered.push((false, l));
        }
        for nl in slots[n_old].drain(..) {
            ordered.push((true, nl));
        }
        let mut cur = Cursor::new(Vec::new());
        let mut offsets: Vec<u32> = Vec::new();
        for (_, l) in &ordered {
            offsets.push(cur.position() as u32);
            l.write_options(&mut cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        }
        let cd_offset = cur.position();
        let mut rebuilt: Vec<CentralDirectoryFileHeader> = Vec::new();
        let mut old_iter = self.files.iter();
        // olds = original count + created parts (fontTable/sheet); news = font files
        let n_new = marks.len();
        let n_created = ordered.len() - self.files.len() - n_new;
        let mut olds_done = 0;
        for (k, (is_new, l)) in ordered.iter().enumerate() {
            if *is_new {
                rebuilt.push(FontWatermark::font_central_for(l, offsets[k]));
            } else if olds_done < self.files.len() {
                let mut c = old_iter.next().cloned().ok_or(Error::TooManyRecords { count: 0 })?;
                c.compressed_size = l.compressed_size;
                c.uncompressed_size = l.uncompressed_size;
                c.crc32 = l.crc32;
                c.local_header_offset = offsets[k];
                rebuilt.push(c);
                olds_done += 1;
            } else {
                // created part (fontTable / sheetWm): file attrs
                let c = FontWatermark::font_central_for(l, offsets[k]);
                let _ = n_created;
                rebuilt.push(c);
            }
        }
        for c in &rebuilt {
            c.write_options(&mut cur, Endian::Little, ()).map_err(|_| Error::Encode)?;
        }
        let cd_size = cur.position() - cd_offset;
        Self::write_eocd32(&mut cur, rebuilt.len() as u16, cd_size as u32, cd_offset as u32)?;
        Ok(cur.into_inner())
    }
}
