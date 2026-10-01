use binrw::binrw;
use sagex_crypto::aes::KeyEncapsulation;

use crate::ecc::{EccChunk, calculate_ecc};

pub const MAGIC_NUMBER: u32 = 0x00106E5A; // This will be written as 5A6E10 in short SAGE X
pub const VERSION: u32 = 1;

/// Number of Reed-Solomon ECC chunks covering `internal`. Infallible wrapper:
/// a failure degrades to zero chunks (still a symmetric file) rather than a
/// panicking writer.
fn ecc_chunk_count(internal: &PrivateInternal) -> u32 {
    calculate_ecc(internal).map(|v| v.len() as u32).unwrap_or(0)
}

/// ECC chunks covering `internal` (same computation as [`ecc_chunk_count`]).
fn ecc_chunks_for(internal: &PrivateInternal) -> Vec<EccChunk> {
    calculate_ecc(internal).unwrap_or_default()
}

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

    // NOTE: the ECC chunks used to be produced via an intermediate
    // `__eccs_computed` temp field with `#[bw(try_calc)]`. binrw *emits*
    // `try_calc` values, so the chunks were written twice (once for the temp
    // field, once for `ecc`) while the reader only consumes one copy —
    // breaking every `.prv` round-trip. The helpers below compute the chunks
    // inline instead, so write and read are structurally symmetric:
    // internal, ecc_len, ecc, identity_len, identity.
    #[bw(calc = ecc_chunk_count(internal))]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    ecc_len: u32,

    #[br(count = ecc_len)]
    #[bw(calc = ecc_chunks_for(internal))]
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

    #[bw(calc = key_dsa.len() as u32)]
    #[br(temp)]
    #[cfg_attr(feature = "serde", serde(skip))]
    dsa_key_len: u32,

    #[br(count = kem_key_len)]
    pub key_kem: Vec<u8>,
    #[br(count = dsa_key_len)]
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

#[cfg(test)]
mod roundtrip_tests {
    use super::*;
    use binrw::{BinRead, BinWriterExt};
    use std::io::Cursor;

    fn tiny_internal(user: &str) -> PrivateInternal {
        PrivateInternal {
            magic_number: MAGIC_NUMBER,
            version: VERSION,
            key_encapsulation_kem: KeyEncapsulation {
                salt: [0xAA; 16],
                nonce_bytes: [0xBB; 12],
                cipher: vec![0xCC; 64],
            },
            key_encapsulation_dsa: KeyEncapsulation {
                salt: [0xDD; 16],
                nonce_bytes: [0xEE; 12],
                cipher: vec![0xFF; 64],
            },
            user_name: user.to_string(),
            key_derivation_mechanism_kem: KeyDerivatinMechanism::Password,
            key_derivation_mechanism_dsa: KeyDerivatinMechanism::Password,
            iteration_count: 7,
        }
    }

    /// Read back must consume the whole buffer: no trailing garbage (double
    /// emission) and no missing prefixes.
    fn read_exact<T>(buf: &[u8], read: impl FnOnce(&mut Cursor<&[u8]>) -> binrw::BinResult<T>) -> T {
        let mut cursor = Cursor::new(buf);
        let value = read(&mut cursor).expect("round-trip read must succeed");
        assert_eq!(
            cursor.position(),
            buf.len() as u64,
            "reader must consume exactly the written bytes"
        );
        value
    }

    #[test]
    fn private_external_roundtrip() {
        let prv = PrivateExternal {
            internal: tiny_internal("alice"),
            identity: b"alice".to_vec(),
        };
        let mut buf = Vec::new();
        Cursor::new(&mut buf).write_le(&prv).expect("write");
        // Sanity: exactly one ECC copy on disk (see note on `ecc_len`).
        let expect_chunks = crate::ecc::calculate_ecc(&prv.internal)
            .expect("ecc")
            .len();
        let back = read_exact(&buf, |c| PrivateExternal::read_le(c));
        assert_eq!(back.internal.user_name, "alice");
        assert_eq!(back.identity, b"alice");
        assert_eq!(back.internal.key_encapsulation_dsa.cipher, vec![0xFF; 64]);
        // Disk layout must be exactly internal + ecc_len + chunks + id_len + id.
        let internal_len = {
            let mut tmp = Vec::new();
            Cursor::new(&mut tmp)
                .write_le(&prv.internal)
                .expect("write internal");
            tmp.len()
        };
        assert_eq!(
            buf.len(),
            internal_len + 4 + expect_chunks * crate::ecc::ECC_LEN + 4 + b"alice".len()
        );
    }

    #[test]
    fn public_external_roundtrip() {
        let publ = PublicFileFormatExternal {
            internal: PublicInternal {
                magic_number: MAGIC_NUMBER,
                version: VERSION,
                key_kem: vec![0x11; 32],
                key_dsa: vec![0x22; 48],
                user_name: "bob".to_string(),
            },
            ecc: vec![],
            identity: b"bob".to_vec(),
        };
        let mut buf = Vec::new();
        Cursor::new(&mut buf).write_le(&publ).expect("write");
        let back = read_exact(&buf, |c| PublicFileFormatExternal::read_le(c));
        assert_eq!(back.internal.user_name, "bob");
        assert_eq!(back.internal.key_kem, vec![0x11; 32]);
        assert_eq!(back.internal.key_dsa, vec![0x22; 48]);
        assert_eq!(back.identity, b"bob");
    }
}
