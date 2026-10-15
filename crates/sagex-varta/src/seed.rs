//! JSON chat seeds — dynamic stress-test messages.
//!
//! Message fixtures live in `assets/seeds/messages.json`, not in code.
//! [`load_messages`] parses the embedded JSON at boot; a malformed file
//! yields an empty list instead of a crash. Colors are `#RRGGBB` strings.

use serde::{Deserialize, Deserializer};

use crate::components::cards::FileKind;

/// `#RRGGBB` color parsed into the `0xRRGGBB` `theme` representation.
#[derive(Clone, Copy)]
pub struct Hex(pub u32);

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let s = s.strip_prefix('#').unwrap_or(&s);
        u32::from_str_radix(s, 16).map(Hex).map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SeedSide {
    Recipient,
    Sender,
}

#[derive(Deserialize, Clone)]
pub struct SeedBadge {
    pub text: String,
    pub fg: Hex,
    pub bg: Hex,
    pub border: Hex,
}

#[derive(Deserialize, Clone, Default)]
pub struct SeedMeta {
    pub text: String,
    #[serde(default)]
    pub color: Option<Hex>,
    #[serde(default)]
    pub bold: bool,
}

#[derive(Deserialize, Clone, Default)]
pub struct SeedPrimary {
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
pub struct SeedScrubber {
    pub progress: f32,
    pub elapsed: String,
    pub total: String,
}

#[derive(Deserialize, Clone, Default)]
pub struct SeedWaveform {
    #[serde(default)]
    pub bars: Vec<f32>,
    pub played: f32,
    pub elapsed: String,
    pub total: String,
}

#[derive(Deserialize, Clone, Default)]
pub struct SeedFooter {
    #[serde(default)]
    pub scrubber: Option<SeedScrubber>,
    #[serde(default)]
    pub waveform: Option<SeedWaveform>,
}

#[derive(Deserialize, Clone, Default)]
pub struct SeedCard {
    pub kind: FileKind,
    pub filename: String,
    #[serde(default)]
    pub chip: Option<String>,
    #[serde(default)]
    pub meta: Vec<SeedMeta>,
    #[serde(default)]
    pub thumb_tag: Option<String>,
    #[serde(default)]
    pub primary: Option<SeedPrimary>,
    #[serde(default)]
    pub icon_action: Option<String>,
    #[serde(default)]
    pub footer: Option<SeedFooter>,
}

#[derive(Deserialize, Clone)]
pub struct SeedMessage {
    pub side: SeedSide,
    pub name: String,
    pub initial: String,
    pub avatar_bg: Hex,
    pub avatar_fg: Hex,
    #[serde(default)]
    pub avatar_border: Option<Hex>,
    pub time: String,
    pub body: String,
    #[serde(default)]
    pub badge: Option<SeedBadge>,
    #[serde(default)]
    pub attachment: Option<SeedCard>,
}

/// Parse the embedded seed JSON. Never panics — empty on any error.
pub fn load_messages() -> Vec<SeedMessage> {
    serde_json::from_str(include_str!("../assets/seeds/messages.json")).unwrap_or_default()
}
