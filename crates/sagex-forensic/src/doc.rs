//! Office document watermarking on top of `office-watermark`.
//!
//! Supported inputs are exactly what `office-watermark` supports:
//! OOXML (`docx/pptx/xlsx`), ODF (`ods/odt/odp`), generic zips.
//! Anything else (PDF, images, text, video) is rejected here with a typed
//! error so callers take the plain-share path — never silently unwatermarked.

use office_watermark::{extract_watermarks_bytes, watermark_bytes, WatermarkOptions};

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("not a watermarkable office document")]
    UnsupportedFormat,
    #[error("watermark engine: {0}")]
    Engine(String),
}

/// True if these bytes look like a supported zip-container office document.
/// Cheap magic pre-check; the engine re-validates on use.
pub fn is_watermarkable(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    // Zip local-file-header magic; office-watermark enforces the rest.
    data[0] == b'P' && data[1] == b'K' && (data[2] == 0x03 || data[2] == 0x05 || data[2] == 0x07)
}

/// Embed `watermark_id` into office bytes. Fails closed on anything the
/// engine cannot handle (caller must NOT store unmarked flagged content).
pub fn embed_watermark(data: &[u8], watermark_id: &str) -> Result<Vec<u8>, DocError> {
    if !is_watermarkable(data) {
        return Err(DocError::UnsupportedFormat);
    }
    let opts =
        WatermarkOptions::new(watermark_id).map_err(|e| DocError::Engine(e.to_string()))?;
    watermark_bytes(data, &opts).map_err(|e| {
        // Normalize "not actually office" into the typed gate error.
        let msg = e.to_string();
        if msg.contains("UnsupportedFormat")
            || msg.contains("unsupported")
            || msg.contains("InvalidArchive")
        {
            DocError::UnsupportedFormat
        } else {
            DocError::Engine(msg)
        }
    })
}

/// Extract the watermark ID from bytes, requiring every hit to agree.
/// Returns `None` when no watermark is present (NOT an error — plain files
/// legitimately have none).
pub fn extract_watermark_id(data: &[u8]) -> Result<Option<String>, DocError> {
    if !is_watermarkable(data) {
        return Ok(None);
    }
    let hits = match extract_watermarks_bytes(data) {
        Ok(h) => h,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UnsupportedFormat")
                || msg.contains("unsupported")
                || msg.contains("InvalidArchive")
            {
                return Ok(None);
            }
            return Err(DocError::Engine(msg));
        }
    };
    if hits.is_empty() {
        return Ok(None);
    }
    let first = &hits[0].value;
    if hits.iter().all(|h| &h.value == first) {
        Ok(Some(first.clone()))
    } else {
        Err(DocError::Engine("conflicting watermarks in one file".into()))
    }
}
