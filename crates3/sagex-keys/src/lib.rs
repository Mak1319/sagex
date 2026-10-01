use sagex_crypto::aes::KeyEncapsulation;
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;
pub const FORMAT_SIGNATURE: [u8; 4] = [0x5A, 0x6E, 0x10, 0x00];
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct InternalKey {
    format_version: u32,
    signature_encapsulation: KeyEncapsulation,
    kem_encapsulation: KeyEncapsulation,
    user_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct KeyConfig {
    signature_algo: String,
    signature_variable: String,
    kem_algo: String,
    kem_variable: String,
}

#[derive(Serialize, Deserialize)]
pub enum PasswordDerivationStrategy {
    Password,
    Hardwere,
}

#[derive(Serialize, Deserialize)]
pub struct EncapsulatedKey {
    pub format_signature: [u8; 4],
    pub version: u32,
    pub key: InternalKey,
    pub key_config: Option<KeyConfig>,
    pub derived_from: PasswordDerivationStrategy,
}

pub struct PublicKey {
    pub format_version: u32,
    pub signature_key: Vec<u8>,
    pub kem_key: Vec<u8>,
    pub user_name: String,
    pub key_config: Option<KeyConfig>,
}

impl From<EncapsulatedKey> for PublicKey {
    fn from(value: EncapsulatedKey) -> Self {
        let internal = value.key;
        let signature_enc = internal.signature_encapsulation;
        let kem_enc = internal.kem_encapsulation;
        PublicKey {
            format_version: internal.format_version,
            signature_key: signature_enc.public_key,
            kem_key: kem_enc.public_key,
            user_name: internal.user_name,
            key_config: value.key_config,
        }
    }
}

impl From<InternalKey> for PublicKey {
    fn from(value: InternalKey) -> Self {
        let signature = value.signature_encapsulation;
        let kem_enc = value.kem_encapsulation;
        PublicKey {
            format_version: value.format_version,
            signature_key: signature.public_key,
            kem_key: kem_enc.public_key,
            user_name: value.user_name,
            key_config: None,
        }
    }
}
