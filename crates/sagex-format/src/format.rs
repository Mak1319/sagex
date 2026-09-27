use binrw::binrw;
use sagex_crypto::aes::KeyEncapsulation;

use crate::ecc::{ECC_LEN, EccChunk, calculate_ecc};

pub const MAGIC_NUMBER: u32 = 0x00106E5A; // This will be written as 5A6E10 in short SAGE X 
pub const VERSION: u32 = 1;

#[binrw]
#[brw(repr=u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum KeyDerivatinMechanism {
    TPM,
    Password,
}

#[binrw]
#[brw(little, magic = 0x00106E5Au32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrivateFileFormatInternal {
    pub magic_number: u32,
    pub version: u32,
    pub key_encapsulation_kem: KeyEncapsulation,
    pub key_encapsulation_dsa: KeyEncapsulation,
    #[bw(calc = user_name.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    user_name_len: u32,

    #[br(count = user_name_len)]
    #[br(map = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).into_owned())]
    #[bw(map = |s: &String| s.as_bytes().to_vec())]
    pub user_name: String,

    pub key_derivation_mechanism_kem: KeyDerivatinMechanism,
    pub key_derivation_mechanism_dsa: KeyDerivatinMechanism,
    #[br(map = |x: u32| x as usize)]
    #[bw(map = |x: &usize| *x as u32)]
    pub iteration_count: usize,
}

#[binrw]
#[brw(little)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrivateFileFormatExternal {
    pub internal: PrivateFileFormatInternal,

    #[br(temp, calc = Vec::new())]
    #[bw(try_calc = calculate_ecc(internal))]
    __eccs_computed: Vec<EccChunk>,

    #[bw(calc = __eccs_computed.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    ecc_len: u32,

    #[br(count = ecc_len)]
    #[bw(calc = __eccs_computed.clone())]
    pub ecc: Vec<EccChunk>,

    #[bw(calc = identity.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    identity_len: u32,

    #[br(count = identity_len)]
    pub identity: Vec<u8>,
}

#[binrw]
#[brw(little, magic = 0x00106E5Au32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PublicFileFormatInternal {
    pub magic_number: u32,
    pub version: u32,
    #[bw(calc = key.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    key_len: u32,

    #[br(count = key_len)]
    pub key: Vec<u8>,

    #[bw(calc = user_name.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    user_name_len: u32,

    #[br(count = user_name_len)]
    #[br(map = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).into_owned())]
    #[bw(map = |s: &String| s.as_bytes().to_vec())]
    pub user_name: String,
}

#[binrw]
#[brw(little)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PublicFileFormatExternal {
    pub internal: PublicFileFormatInternal,
    #[bw(calc = ecc.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    ecc_len: u32,

    #[br(count = ecc_len)]
    pub ecc: Vec<u8>,

    #[bw(calc = identity.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    identity_len: u32,

    #[br(count = identity_len)]
    pub identity: Vec<u8>,
}
