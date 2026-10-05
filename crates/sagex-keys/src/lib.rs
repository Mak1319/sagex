use sagex_crypto::aes::{KeyEncapsulation, key_derivation::KeyDerivation};
use serde::{Deserialize, Serialize};

use crate::error::{
    KeyError::{self, DsaDerivationError, KemDerivationError},
    KeyVarient::{Dsa, Kem},
};

mod error;

pub type MagicBytes = [u8; 4];

pub const FORMAT_VERSION: u32 = 1;
pub const KEY_FORMAT_MAGIC_BYTES: MagicBytes = [0x5A, 0x6E, 0x10, 0x00];



#[derive(Serialize, Deserialize)]
pub struct KeyInternal {
    format_version: u32,
    user_name: String,
    user_others: String,
    dsa_key: KeyEncapsulation,
    kem_key: KeyEncapsulation,
}


impl KeyInternal {
    pub fn new(
        plain_password: &[u8],
        user_name: String,
        user_other: String,
    ) -> Result<Self, KeyError> {
        let salt_bytes = KeyDerivation::get_salt();
        let kdf_key = KeyDerivation::derive_password(plain_password, salt_bytes);

        let dsa_enc =
            KeyEncapsulation::from_key::<sagex_crypto::aes::dsa::MlDsa65>(kdf_key, salt_bytes)
                .map_err(|e| DsaDerivationError(e))?;
        let kem_enc =
            KeyEncapsulation::from_key::<sagex_crypto::aes::kem::MlKem768>(kdf_key, salt_bytes)
                .map_err(|e| KemDerivationError(e))?;

        Ok(Self {
            format_version: FORMAT_VERSION,
            user_name,
            user_others: user_other,
            dsa_key: dsa_enc,
            kem_key: kem_enc,
        })
    }
}





#[derive(Serialize, Deserialize)]
pub enum DerivationStrategy {
    Softwere,
    Hardwere,
    Password,
}

impl Default for DerivationStrategy {
    fn default() -> Self {
        Self::Softwere
    }
}





#[derive(Serialize, Deserialize)]
pub struct PublicKeyInternal {
    pub format_version: u32,
    pub user_name: String,
    pub user_others: String,
    pub dsa_key: Vec<u8>,
    pub kem_key: Vec<u8>,
}

impl TryFrom<KeyInternal> for PublicKeyInternal {
    type Error = crate::error::KeyError;
    fn try_from(value: KeyInternal) -> Result<Self, Self::Error> {
        let Some(dsa_key) = value.dsa_key.public_key else {
            return Err(KeyError::PublicKeyNotProvided(Dsa));
        };

        let Some(kem_key) = value.kem_key.public_key else {
            return Err(KeyError::PublicKeyNotProvided(Kem));
        };

        Ok(Self {
            format_version: FORMAT_VERSION,
            user_name: value.user_name,
            user_others: value.user_others,
            dsa_key,
            kem_key,
        })
    }
}




#[derive(Serialize, Deserialize)]
pub enum KeyType {
    Private(KeyInternal),
    Public(PublicKeyInternal),
}

#[derive(Serialize, Deserialize)]
pub struct KeyExternal {
    pub format_magic: MagicBytes,
    pub internals: KeyType,
    pub key_derivation_strategy: DerivationStrategy,
}
