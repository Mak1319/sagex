//! HTTP client for `sagex-certauth`: CSR submit, certificate fetch/verify.
//! Mirrors the `ApiClient` style (async, runs on the network runtime).

use serde::{Deserialize, Serialize};

use super::client::{ApiResult, BackendError};

#[derive(Debug, Clone)]
pub struct CaClient {
    inner: reqwest::Client,
    base: String,
}

#[derive(Debug, Clone, Serialize)]
struct CsrRequest<'a> {
    permit: &'a str,
    csr: &'a super::identity::CsrBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    chatsrv_token: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Permit {
    pub permit: String,
    pub sub: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CertIssued {
    pub identity: String,
    pub serial: u64,
    pub certificate_pem: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CertVerify {
    pub serial: u64,
    pub identity: String,
    pub valid: bool,
}

impl CaClient {
    pub fn new(base: &str) -> Self {
        Self {
            inner: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("reqwest client"),
            base: base.trim_end_matches('/').to_string(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
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
            .unwrap_or_else(|| format!("CA request failed ({code})"));
        BackendError {
            status: Some(code),
            message: msg,
        }
    }

    pub async fn health(&self) -> ApiResult<()> {
        let res = self.inner.get(self.url("/health")).send().await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    /// Request a self-service user permit using the chatsrv access token.
    /// The CA verifies the token and binds `sub` to its username — callers
    /// can never choose another subject. Requires the token, not a password.
    pub async fn request_permit(&self, access_token: &str) -> ApiResult<Permit> {
        let res = self
            .inner
            .post(self.url("/v1/permits"))
            .bearer_auth(access_token)
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    /// Submit a CSR with a permit. Pass the chatsrv access token when
    /// available so the CA triple-binds identity (required in strict mode).
    pub async fn submit_csr(
        &self,
        permit: &str,
        csr: &super::identity::CsrBody,
        chatsrv_token: Option<&str>,
    ) -> ApiResult<CertIssued> {
        let res = self
            .inner
            .post(self.url("/v1/csr"))
            .json(&CsrRequest {
                permit,
                csr,
                chatsrv_token,
            })
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn fetch_cert(&self, serial: u64) -> ApiResult<CertIssued> {
        #[derive(Deserialize)]
        struct CertRecord {
            identity: String,
            serial: u64,
            certificate_pem: String,
        }
        let res = self
            .inner
            .get(self.url(&format!("/v1/certs/{serial}")))
            .send()
            .await?;
        if res.status().is_success() {
            let rec: CertRecord = res.json().await.map_err(BackendError::from)?;
            return Ok(CertIssued {
                identity: rec.identity,
                serial: rec.serial,
                certificate_pem: rec.certificate_pem,
            });
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn verify_cert(&self, serial: u64) -> ApiResult<CertVerify> {
        let res = self
            .inner
            .get(self.url(&format!("/v1/certs/{serial}/verify")))
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

    #[test]
    fn csr_request_shape_matches_server() {
        fn body() -> crate::backend::identity::CsrBody {
            crate::backend::identity::CsrBody {
                identity: "alice".into(),
                key_kem_b64: "k".into(),
                key_dsa_b64: "d".into(),
                self_sig_b64: "s".into(),
            }
        }
        // chatsrv_token present -> triple-bind path.
        let v = serde_json::to_value(&CsrRequest {
            permit: "p".into(),
            csr: &body(),
            chatsrv_token: Some("t".into()),
        })
        .unwrap();
        assert_eq!(v["chatsrv_token"], "t");
        assert_eq!(v["csr"]["identity"], "alice");
        // Absent -> key omitted entirely (legacy permit+PoP path).
        let v = serde_json::to_value(&CsrRequest {
            permit: "p".into(),
            csr: &body(),
            chatsrv_token: None,
        })
        .unwrap();
        assert!(v.get("chatsrv_token").is_none());
    }

    #[test]
    fn permit_response_shape() {
        let v: Permit = serde_json::from_value(serde_json::json!({
            "permit": "p", "sub": "alice", "expires_at": 123,
        }))
        .unwrap();
        assert_eq!(v.sub, "alice");
    }
}
