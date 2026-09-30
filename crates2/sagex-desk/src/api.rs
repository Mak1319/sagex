//! Blocking REST client for sagex-chatsrv. Calls run synchronously
//! (localhost sample app); `net::run_bg` is the single choke point if
//! this ever moves to worker threads.
#![allow(dead_code)] // DTO fields mirror the server; some are future-use.

use serde::{Deserialize, Serialize};

// ---------- DTOs (mirror the server JSON) ----------

#[derive(Debug, Clone, Deserialize)]
pub struct LoginResp {
    pub access_token: String,
    pub refresh_token: String,
    pub object_key: String,
    pub username: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterResp {
    pub object_key: String,
    pub username: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshResp {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Member {
    pub object_key: String,
    pub username: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    pub object_key: String,
    pub name: String,
    pub members: Vec<Member>,
    pub admins: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub object_key: String,
    pub sender_key: String,
    pub body: String,
    pub content_type: Option<String>,
    pub rustfs_ref: Option<String>,
    pub created_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadUrl {
    pub url: String,
    pub key: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadUrl {
    pub url: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceToken {
    pub token_type: String,
    pub payload: String,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PubkeyResp {
    pub key_dsa: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub base_url: String,
    pub object_key: String,
    pub username: String,
    pub access_token: String,
    pub refresh_token: String,
}

// ---------- client ----------

pub struct Api {
    base: String,
    http: reqwest::blocking::Client,
}

fn api_error(resp: reqwest::blocking::Response) -> String {
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    let short: String = body.chars().take(200).collect();
    format!("{status}: {short}")
}

impl Api {
    pub fn new(base: &str, timeout_secs: u64) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs.max(1)))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    pub fn register(
        &self,
        username: &str,
        password: &str,
    ) -> Result<RegisterResp, String> {
        let resp = self
            .http
            .post(self.url("/auth/register"))
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            resp.json().map_err(|e| e.to_string())
        } else {
            Err(api_error(resp))
        }
    }

    pub fn login(&self, username: &str, password: &str) -> Result<LoginResp, String> {
        let resp = self
            .http
            .post(self.url("/auth/login"))
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            resp.json().map_err(|e| e.to_string())
        } else {
            Err(api_error(resp))
        }
    }

    /// POST with Bearer auth + one refresh-and-retry on 401.
    fn post_authed<T: for<'de> Deserialize<'de>>(
        &self,
        session: &mut Session,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, String> {
        let mut resp = self
            .http
            .post(self.url(path))
            .bearer_auth(&session.access_token)
            .json(body)
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.refresh(session)?;
            resp = self
                .http
                .post(self.url(path))
                .bearer_auth(&session.access_token)
                .json(body)
                .send()
                .map_err(|e| e.to_string())?;
        }
        if resp.status().is_success() {
            resp.json().map_err(|e| e.to_string())
        } else {
            Err(api_error(resp))
        }
    }

    /// GET with Bearer auth + one refresh-and-retry on 401.
    fn get_authed<T: for<'de> Deserialize<'de>>(
        &self,
        session: &mut Session,
        path: &str,
    ) -> Result<T, String> {
        let mut resp = self
            .http
            .get(self.url(path))
            .bearer_auth(&session.access_token)
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.refresh(session)?;
            resp = self
                .http
                .get(self.url(path))
                .bearer_auth(&session.access_token)
                .send()
                .map_err(|e| e.to_string())?;
        }
        if resp.status().is_success() {
            resp.json().map_err(|e| e.to_string())
        } else {
            Err(api_error(resp))
        }
    }

    fn refresh(&self, session: &mut Session) -> Result<(), String> {
        let resp = self
            .http
            .post(self.url("/auth/refresh"))
            .json(&serde_json::json!({ "refresh_token": session.refresh_token }))
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            let r: RefreshResp = resp.json().map_err(|e| e.to_string())?;
            session.access_token = r.access_token;
            session.refresh_token = r.refresh_token.clone();
            // Persist the rotated refresh token; ignore store failures
            // (memory session still works until restart).
            let _ = crate::secrets::store_refresh(&session.username, &r.refresh_token);
            Ok(())
        } else {
            Err("session expired, sign in again".to_string())
        }
    }

    pub fn groups(&self, session: &mut Session) -> Result<Vec<Group>, String> {
        self.get_authed(session, "/groups")
    }

    pub fn create_group(
        &self,
        session: &mut Session,
        name: &str,
        members: &[String],
    ) -> Result<Group, String> {
        self.post_authed(
            session,
            "/groups",
            &serde_json::json!({ "name": name, "members": members }),
        )
    }

    pub fn add_members(
        &self,
        session: &mut Session,
        group_id: &str,
        members: &[String],
    ) -> Result<Group, String> {
        self.post_authed(
            session,
            &format!("/groups/{group_id}/members"),
            &serde_json::json!({ "members": members }),
        )
    }

    pub fn remove_member(
        &self,
        session: &mut Session,
        group_id: &str,
        user_key: &str,
    ) -> Result<serde_json::Value, String> {
        let mut resp = self
            .http
            .delete(self.url(&format!("/groups/{group_id}/members/{user_key}")))
            .bearer_auth(&session.access_token)
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.refresh(session)?;
            resp = self
                .http
                .delete(self.url(&format!("/groups/{group_id}/members/{user_key}")))
                .bearer_auth(&session.access_token)
                .send()
                .map_err(|e| e.to_string())?;
        }
        if resp.status().is_success() {
            resp.json().map_err(|e| e.to_string())
        } else {
            Err(api_error(resp))
        }
    }

    pub fn messages(
        &self,
        session: &mut Session,
        group_id: &str,
    ) -> Result<Vec<Message>, String> {
        self.get_authed(session, &format!("/groups/{group_id}/messages?limit=100"))
    }

    pub fn post_message(
        &self,
        session: &mut Session,
        group_id: &str,
        body: &str,
    ) -> Result<Message, String> {
        self.post_authed(
            session,
            &format!("/groups/{group_id}/messages"),
            &serde_json::json!({ "body": body, "content_type": "text/e2ee-placeholder" }),
        )
    }

    pub fn upload_url(
        &self,
        session: &mut Session,
        filename: &str,
    ) -> Result<UploadUrl, String> {
        self.post_authed(
            session,
            "/documents/upload-url",
            &serde_json::json!({ "filename": filename }),
        )
    }

    pub fn put_bytes(&self, url: &str, bytes: Vec<u8>) -> Result<(), String> {
        let resp = self
            .http
            .put(url)
            .body(bytes)
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(api_error(resp))
        }
    }

    pub fn download_url(
        &self,
        session: &mut Session,
        key: &str,
    ) -> Result<DownloadUrl, String> {
        self.get_authed(session, &format!("/documents/download-url?key={key}"))
    }

    pub fn ca_token(&self, session: &mut Session) -> Result<ServiceToken, String> {
        self.post_authed(session, "/ca/token", &serde_json::json!({}))
    }

    pub fn service_pubkey(&self) -> Result<PubkeyResp, String> {
        let resp = self
            .http
            .get(self.url("/service/pubkey"))
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            resp.json().map_err(|e| e.to_string())
        } else {
            Err(api_error(resp))
        }
    }
}
