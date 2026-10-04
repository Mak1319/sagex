use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sagex_keys::PublicKey;
use serde::{Deserialize, Serialize};

use error::CsrError;

pub mod error;
mod timestamp;

#[derive(Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct CertificateSigningRequest {
    pub payload: PublicKey,
}

impl CertificateSigningRequest {
    pub fn to_bytes(&self) -> Result<Vec<u8>, CsrError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|v| v.to_vec())
            .map_err(CsrError::Encode)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CsrError> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes).map_err(CsrError::Decode)
    }
}

#[derive(Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ProveOfOrigin {
    pub user_name: String,
    #[rkyv(with = timestamp::SecsNanos)]
    pub iat: DateTime<Utc>,
    #[rkyv(with = timestamp::SecsNanos)]
    pub exat: DateTime<Utc>,
    pub iss: String,
    pub operation: String,
    pub permissions: HashMap<String, String>,
    pub signature: Vec<u8>,
}

impl ProveOfOrigin {
    pub fn to_bytes(&self) -> Result<Vec<u8>, CsrError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|v| v.to_vec())
            .map_err(CsrError::Encode)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CsrError> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes).map_err(CsrError::Decode)
    }
}
