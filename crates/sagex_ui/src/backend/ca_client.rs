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

    /// Submit a CSR with a pasted (operator-issued) permit.
    pub async fn submit_csr(
        &self,
        permit: &str,
        csr: &super::identity::CsrBody,
    ) -> ApiResult<CertIssued> {
        let res = self
            .inner
            .post(self.url("/v1/csr"))
            .json(&CsrRequest { permit, csr })
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
