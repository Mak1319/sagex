use binrw::binrw;
use sagex_crypto::aes::KeyEncapsulation;

use crate::ecc::{EccChunk, calculate_ecc};

pub const MAGIC_NUMBER: u32 = 0x00106E5A; // This will be written as 5A6E10 in short SAGE X 
pub const VERSION: u32 = 1;

#[binrw]
#[brw(repr=u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum KeyDerivatinMechanism {
    TPM,
    Password,
}

/// Internal structure of an key
#[binrw]
#[brw(little)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrivateInternal {
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
pub struct PrivateExternal {
    pub internal: PrivateInternal,

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
#[brw(little)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PublicInternal {
    pub magic_number: u32,
    pub version: u32,
    #[bw(calc = key_kem.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    kem_key_len: u32,

    #[bw(calc = key_kem.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    dsa_key_len: u32,

    #[br(count = kem_key_len)]
    pub key_kem: Vec<u8>,
    #[br(count = kem_key_len)]
    pub key_dsa: Vec<u8>,

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
    pub internal: PublicInternal,
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

pub mod certificative {}
