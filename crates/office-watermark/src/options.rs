use crate::error::{Error, Result};

/// Namespace prefix used for the watermark attribute.
pub const WM_PREFIX: &str = "sagex";
/// Qualified watermark attribute name, e.g. `sagex:wm`.
pub const WM_ATTR: &str = "sagex:wm";
/// Namespace URI declared as `xmlns:sagex`.
pub const WM_NS: &str = "https://sagex/enc";
/// Declaration attribute name.
pub const WM_XMLNS_ATTR: &str = "xmlns:sagex";

/// Maximum watermark length in bytes (after trimming). Keeps OOXML/ODF parts small.
pub const MAX_WATERMARK_LEN: usize = 4096;

/// Options for watermark injection. Only the text value varies per call;
/// attribute name/namespace are fixed to `sagex:wm` per spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatermarkOptions {
    value: String,
}

impl WatermarkOptions {
    /// Build options from a text watermark.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(Error::InvalidOptions("watermark must not be empty".into()));
        }
        if value.len() > MAX_WATERMARK_LEN {
            return Err(Error::InvalidOptions(format!(
                "watermark too long ({} > {MAX_WATERMARK_LEN} bytes)",
                value.len()
            )));
        }
        // Reject ASCII control chars (except tab/newline/CR) which are illegal in XML 1.0 attrs.
        if value
            .chars()
            .any(|c| c.is_control() && c != '\t' && c != '\n' && c != '\r')
        {
            return Err(Error::InvalidOptions(
                "watermark contains illegal XML control characters".into(),
            ));
        }
        Ok(Self { value })
    }

    /// Raw watermark text.
    pub fn value(&self) -> &str {
        &self.value
    }
}
