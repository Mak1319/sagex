//! Blocking read-only REST client for the sagex-register-gateway
//! telemetry API. Lookup only — there is intentionally NO register/post
//! path in this app.

use serde::Deserialize;

// ---------- DTOs (mirror the gateway/node JSON) ----------

#[derive(Debug, Clone, Deserialize)]
pub struct Record {
    pub recipient_id: String,
    pub watermark_id: String,
    pub doc_hash: String,
    pub recipient_dsa_pub: String,
    pub session_id: String,
    pub timestamp_ms: i64,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Block {
    pub height: u64,
    pub prev_hash: String,
    pub record: Record,
    pub block_hash: String,
}

// ---------- client ----------

pub struct Api {
    base: String,
    bearer: String,
    http: reqwest::blocking::Client,
}

fn api_error(resp: reqwest::blocking::Response) -> String {
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    let short: String = body.chars().take(200).collect();
    format!("{status}: {short}")
}

impl Api {
    pub fn new(base: &str, bearer: &str, timeout_secs: u64) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs.max(1)))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            bearer: bearer.to_string(),
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, String> {
        let mut req = self.http.get(self.url(path));
        if !self.bearer.is_empty() {
            req = req.bearer_auth(&self.bearer);
        }
        let resp = req.send().map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            resp.json().map_err(|e| e.to_string())
        } else {
            Err(api_error(resp))
        }
    }

    pub fn health(&self) -> Result<String, String> {
        let resp = self
            .http
            .get(self.url("/health"))
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(resp.text().unwrap_or_default())
        } else {
            Err(api_error(resp))
        }
    }

    /// Full chain (lookup).
    pub fn records(&self) -> Result<Vec<Block>, String> {
        self.get("/records")
    }

    /// Single block by watermark id (lookup).
    pub fn record(&self, watermark_id: &str) -> Result<Block, String> {
        self.get(&format!("/records/{watermark_id}"))
    }
}
