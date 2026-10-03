use sagex_crypto::aes::{KEY_LEN, KeyEncapsulation, SALT_LEN};
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;
pub const FORMAT_SIGNATURE: [u8; 4] = [0x5A, 0x6E, 0x10, 0x00];
pub const VERSION: u32 = 1;

/// It is the internal structure of a keys which need to be stored
/// in the storage for future use
#[derive(Serialize, Deserialize)]
pub struct InternalKey {
    format_version: u32,
    signature_encapsulation: KeyEncapsulation,
    kem_encapsulation: KeyEncapsulation,
    user_name: String,
}

impl InternalKey {
    /// This is the maker for a fresh user key with password.
    ///
    /// What it does it makes a new signing key and a new kem key, locks
    /// both of them with the password and keeps the user name with them.
    pub fn new(
        password: &[u8],
        user_name: String,
    ) -> Result<Self, sagex_crypto::error::SagexCrypotError> {
        use sagex_crypto::{MlDsa44, MlKem768};
        Ok(Self {
            format_version: FORMAT_VERSION,
            signature_encapsulation: KeyEncapsulation::new::<MlDsa44>(password)?,
            kem_encapsulation: KeyEncapsulation::new::<MlKem768>(password)?,
            user_name,
        })
    }

    pub fn get_kem_deliverable(&self) -> ([u8; SALT_LEN], u32) {
        (self.kem_encapsulation.salt, self.kem_encapsulation.rounds)
    }

    /// This is the getter for the user name kept inside.
    ///
    /// What it does it gives back the name this key belongs to.
    pub fn user_name(&self) -> &str {
        &self.user_name
    }

    pub fn get_signature_deliverable(&self) -> ([u8; SALT_LEN], u32) {
        (
            self.signature_encapsulation.salt,
            self.signature_encapsulation.rounds,
        )
    }

    pub fn get_signature_key(
        &self,
        aes_key: [u8; KEY_LEN],
    ) -> Result<sagex_crypto::aes::KeyDerivedEncapsulation, sagex_crypto::error::SagexCrypotError>
    {
        return self.signature_encapsulation.decrypt_from_password(aes_key);
    }
    pub fn get_kem_key(
        &self,
        aes_key: [u8; KEY_LEN],
    ) -> Result<sagex_crypto::aes::KeyDerivedEncapsulation, sagex_crypto::error::SagexCrypotError>
    {
        return self.kem_encapsulation.decrypt_from_password(aes_key);
    }
}

/// Stores other information which are not important for cryptography
///
/// Its most relevant use is in future
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

/// This is the encapsulation around the internal structure
/// it this is the exact format which will be stored in the file
#[derive(Serialize, Deserialize)]
pub struct EncapsulatedKey {
    pub format_signature: [u8; 4],
    pub version: u32,
    pub key: InternalKey,
    pub key_config: Option<KeyConfig>,
    pub derived_from: PasswordDerivationStrategy,
}

/// This only encapsulate the public keys in a structure
///
/// Useful when CSR have to made or visit through internet
#[derive(Serialize, Deserialize)]
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
