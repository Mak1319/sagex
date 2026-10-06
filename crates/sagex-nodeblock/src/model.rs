use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LedgerRecord {
    pub watermark: String,
    pub file_hash: String,
    pub username: String,
    pub certificate: String,
    pub filename: String,
    pub session_id: String,
    #[serde(default)]
    pub recipient_signature: String,
    #[serde(default)]
    pub file_signature: String,
    pub event_time: String,
}

impl LedgerRecord {
    pub fn validate_for_storage(&self) -> Result<(), &'static str> {
        if self.watermark.is_empty()
            || self.file_hash.is_empty()
            || self.username.is_empty()
            || self.certificate.is_empty()
            || self.filename.is_empty()
            || self.session_id.is_empty()
            || self.event_time.is_empty()
        {
            return Err("required record fields must not be empty");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Block {
    pub height: u64,
    pub previous_hash: String,
    pub block_hash: String,
    pub record: LedgerRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Vote {
    pub validator_id: String,
    pub block_hash: String,
    pub signature_hex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommitCertificate {
    pub height: u64,
    pub block_hash: String,
    pub votes: Vec<Vote>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommittedRecord {
    pub block: Block,
    pub certificate: CommitCertificate,
}
