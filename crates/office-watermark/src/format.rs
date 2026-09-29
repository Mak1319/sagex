/// Office container format detected from zip entry names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeFormat {
    Docx,
    Pptx,
    Xlsx,
    Ods,
    Odt,
    Odp,
    /// Zip with XML inside but no recognised Office marker.
    GenericZip,
}

/// Detect format from a list of zip entry names.
///
/// Never fails: unknown layouts fall back to `GenericZip` so the caller can
/// still watermark every embedded XML file.
pub fn detect_from_names(names: &[String]) -> OfficeFormat {
    let has = |needle: &str| names.iter().any(|n| n == needle || n.starts_with(needle));
    let has_content_types = names.iter().any(|n| n == "[Content_Types].xml");
    let has_mimetype = names.iter().any(|n| n == "mimetype");

    if has_content_types {
        if has("word/") {
            return OfficeFormat::Docx;
        }
        if has("ppt/") {
            return OfficeFormat::Pptx;
        }
        if has("xl/") {
            return OfficeFormat::Xlsx;
        }
        return OfficeFormat::GenericZip;
    }

    if has_mimetype || has("META-INF/manifest.xml") {
        // Distinguish ODF flavours by mimetype payload when possible is done
        // by the caller; by names alone we default to Ods-family markers.
        if has("content.xml") {
            // Could be Ods/Odt/Odp; default to Ods unless caller refines.
            return OfficeFormat::Ods;
        }
        return OfficeFormat::GenericZip;
    }

    // Heuristic fallbacks when markers are missing (e.g. truncated listing).
    if has("word/") {
        return OfficeFormat::Docx;
    }
    if has("ppt/") {
        return OfficeFormat::Pptx;
    }
    if has("xl/") {
        return OfficeFormat::Xlsx;
    }

    OfficeFormat::GenericZip
}

/// Refine ODF flavour from the `mimetype` file payload.
pub fn refine_odf_from_mimetype(payload: &str) -> OfficeFormat {
    if payload.contains("spreadsheet") {
        OfficeFormat::Ods
    } else if payload.contains("presentation") {
        OfficeFormat::Odp
    } else if payload.contains("text") {
        OfficeFormat::Odt
    } else {
        OfficeFormat::Ods
    }
}

/// Legacy OLE (password-protected / pre-OOXML) magic: D0 CF 11 E0.
pub fn is_ole_magic(header: &[u8]) -> bool {
    header.len() >= 4
        && header[0] == 0xD0
        && header[1] == 0xCF
        && header[2] == 0x11
        && header[3] == 0xE0
}

/// Zip local-file-header magic: PK\x03\x04 (also accepts empty-span/directory variants).
pub fn is_zip_magic(header: &[u8]) -> bool {
    header.len() >= 4
        && header[0] == b'P'
        && header[1] == b'K'
        && (header[2] == 0x03 || header[2] == 0x05 || header[2] == 0x07)
        && (header[3] == 0x04 || header[3] == 0x06 || header[3] == 0x08)
}
