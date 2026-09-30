//! Minimal client for the registration authority (`sagex-gateway`):
//! watermark record intake + lookup by watermark. Used by the forensic
//! send/receive pipeline; screens wire it up later.

use serde::{Deserialize, Serialize};

use super::client::{ApiResult, BackendError};

#[derive(Debug, Clone)]
pub struct GatewayClient {
    inner: reqwest::Client,
    base: String,
}

/// A decryption record exactly as the ledger models it. Field names must
/// match `sagex-ledger/src/model.rs` (covered by `record_shape` test).
#[derive(Debug, Clone, Serialize)]
pub struct DecryptionRecord {
    pub watermark: String,
    pub session_id: String,
    pub timestamp: i64,
    pub user_id: String,
    pub file_hash: String,
    pub payload_hash: String,
    pub auth_server: String,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitResult {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub block_index: Option<u64>,
    #[serde(default)]
    pub block_hash: Option<String>,
    #[serde(default)]
    pub duplicate: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LookupResult {
    #[serde(default)]
    pub blocks: Vec<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

impl GatewayClient {
    pub fn new(base: &str) -> Self {
        Self {
            inner: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("reqwest client"),
            base: base.trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    async fn err_of(status: reqwest::StatusCode, res: reqwest::Response) -> BackendError {
        let code = status.as_u16();
        #[derive(Deserialize)]
        struct ErrorBody {
            error: Option<String>,
        }
        let msg = res
            .json::<ErrorBody>()
            .await
            .ok()
            .and_then(|b| b.error)
            .unwrap_or_else(|| format!("Gateway request failed ({code})"));
        BackendError {
            status: Some(code),
            message: msg,
        }
    }

    /// Submit a record. `permit` is optional: with auth enabled the gateway
    /// requires `{permit, record}` and burns the JTI; pass `None` only
    /// against open demo deployments.
    pub async fn submit(
        &self,
        permit: Option<&str>,
        record: &DecryptionRecord,
    ) -> ApiResult<SubmitResult> {
        let body = match permit {
            Some(p) => serde_json::json!({ "permit": p, "record": record }),
            None => serde_json::to_value(record).map_err(|e| BackendError {
                status: None,
                message: format!("encode record: {e}"),
            })?,
        };
        let res = self
            .inner
            .post(self.url("/register"))
            .json(&body)
            .send()
            .await?;
        // 201 committed, 202 queued, 200 duplicate are all non-error here;
        // the caller inspects `status`.
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    /// Look up blocks by watermark (leak attribution).
    pub async fn lookup(&self, watermark: &str) -> ApiResult<LookupResult> {
        let res = self
            .inner
            .get(self.url(&format!("/record/{watermark}")))
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Record JSON keys must match the ledger model exactly.
    #[test]
    fn record_shape_matches_ledger() {
        let r = DecryptionRecord {
            watermark: "wm-x".into(),
            session_id: "s".into(),
            timestamp: 1,
            user_id: "u".into(),
            file_hash: "a".into(),
            payload_hash: "b".into(),
            auth_server: "auth-1".into(),
            signature: "sig".into(),
        };
        let v = serde_json::to_value(&r).unwrap();
        for k in [
            "watermark",
            "session_id",
            "timestamp",
            "user_id",
            "file_hash",
            "payload_hash",
            "auth_server",
            "signature",
        ] {
            assert!(v.get(k).is_some(), "missing key {k}");
        }
    }
}
