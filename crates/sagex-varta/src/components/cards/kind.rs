//! File-kind data for chat cards.
//!
//! A kind carries its chip label, accent tint (bg/border/fg) and glyph.
//! New file types add a variant here — no layout code changes.

/// Chat-card file kind.
#[derive(Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    #[default]
    Pdf,
    Image,
    Video,
    Audio,
    Sheet,
    Slides,
    Doc,
}

impl FileKind {
    /// Uppercase chip label shown beside the filename.
    pub fn chip(&self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Image => "IMG // 2D",
            Self::Video => "H.264",
            Self::Audio => "WAV // 48kHz",
            Self::Sheet => "XLSX // SPREADSHEET",
            Self::Slides => "PPTX // SLIDES",
            Self::Doc => "DOCX // SPEC",
        }
    }

    /// Accent tint as `(background, border, foreground)`.
    pub fn tint(&self) -> (u32, u32, u32) {
        match self {
            Self::Pdf => (0xfef2f2, 0xfecaca, 0xb91c1c),
            Self::Image => (0xf0f9ff, 0xbae6fd, 0x0369a1),
            Self::Video => (0xfaf5ff, 0xe9d5ff, 0x7e22ce),
            Self::Audio => (0xecfdf5, 0xa7f3d0, 0x047857),
            Self::Sheet => (0xecfdf5, 0xa7f3d0, 0x047857),
            Self::Slides => (0xfffbeb, 0xfde68a, 0xb45309),
            Self::Doc => (0xeff6ff, 0xbfdbfe, 0x1d4ed8),
        }
    }

    /// Glyph drawn in the leading kind box.
    pub fn glyph(&self) -> &'static str {
        match self {
            Self::Pdf | Self::Doc => "icons/doc.svg",
            Self::Image => "icons/photo.svg",
            Self::Video => "icons/video-camera.svg",
            Self::Audio => "icons/audio.svg",
            Self::Sheet => "icons/sheet.svg",
            Self::Slides => "icons/slides.svg",
        }
    }
}
