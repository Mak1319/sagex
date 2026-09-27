use binrw::binrw;

#[binrw]
#[brw(little)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AppConfig {
    #[bw(calc = key_location.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    key_location_len: u32,

    #[br(count = key_location_len)]
    #[br(map = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).into_owned())]
    #[bw(map = |s: &String| s.as_bytes().to_vec())]
    pub key_location: String,

    #[bw(calc = key_current_key_identity.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    key_current_key_identity_len: u32,

    #[br(count = key_current_key_identity_len)]
    #[br(map = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).into_owned())]
    #[bw(map = |s: &String| s.as_bytes().to_vec())]
    pub key_current_key_identity: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        return Self {
            key_location: "~/.sagex".into(),
            key_current_key_identity: "sagex".into(),
        };
    }
}
