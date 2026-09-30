//! Async REST client for `sagex-chatsrv`. Every method is `async` and runs
//! on the dedicated network runtime (see `runtime.rs`) — never on GPUI.

use reqwest::StatusCode;

use super::types::*;

#[derive(Debug, Clone)]
pub struct BackendError {
    pub status: Option<u16>,
    pub message: String,
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<reqwest::Error> for BackendError {
    fn from(e: reqwest::Error) -> Self {
        Self {
            status: e.status().map(|s| s.as_u16()),
            message: if e.is_connect() || e.is_timeout() {
                "Cannot reach the chat server. Is it running?".to_string()
            } else {
                format!("Network error: {e}")
            },
        }
    }
}

pub type ApiResult<T> = Result<T, BackendError>;

#[derive(Debug, Clone)]
pub struct ApiClient {
    inner: reqwest::Client,
    base: String,
}

impl ApiClient {
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

    async fn check_empty(&self, res: reqwest::Response) -> ApiResult<serde_json::Value> {
        let status = res.status();
        if status.is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(status, res).await)
    }

    async fn err_of(status: StatusCode, res: reqwest::Response) -> BackendError {
        let code = status.as_u16();
        let msg = res
            .json::<ApiErrorBody>()
            .await
            .ok()
            .and_then(|b| b.error)
            .unwrap_or_else(|| format!("Request failed ({code})"));
        BackendError {
            status: Some(code),
            message: msg,
        }
    }

    // ---------- health ----------
    pub async fn health(&self) -> ApiResult<()> {
        let res = self.inner.get(self.url("/health")).send().await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    // ---------- auth ----------
    pub async fn signup(
        &self,
        email: &str,
        username: &str,
        password: &str,
        display_name: Option<&str>,
    ) -> ApiResult<SignupRes> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/signup"))
            .json(&SignupReq {
                email: email.to_string(),
                username: username.to_string(),
                password: password.to_string(),
                display_name: display_name.map(|s| s.to_string()),
            })
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    /// Verify OTP. `Ok(Tokens)` for signup/login purposes; reset purpose
    /// returns the raw JSON (caller checks `reset_allowed`).
    pub async fn verify_otp_raw(
        &self,
        email: &str,
        code: &str,
        purpose: &str,
    ) -> ApiResult<serde_json::Value> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/verify-otp"))
            .json(&VerifyOtpReq {
                email: email.into(),
                code: code.into(),
                purpose: purpose.into(),
            })
            .send()
            .await?;
        self.check_empty(res).await
    }

    pub fn tokens_of(v: &serde_json::Value) -> Option<Tokens> {
        serde_json::from_value(v.clone()).ok()
    }

    pub async fn request_otp(&self, email: &str, purpose: &str) -> ApiResult<()> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/request-otp"))
            .json(&RequestOtpReq {
                email: email.into(),
                purpose: purpose.into(),
            })
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    pub async fn login(&self, email: &str, password: &str) -> ApiResult<Tokens> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/login"))
            .json(&LoginReq {
                email: email.into(),
                password: password.into(),
            })
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn refresh(&self, refresh_token: &str) -> ApiResult<Tokens> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/refresh"))
            .json(&RefreshReq {
                refresh_token: refresh_token.into(),
            })
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn logout(&self, access: &str, refresh_token: &str) -> ApiResult<()> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/logout"))
            .bearer_auth(access)
            .json(&RefreshReq {
                refresh_token: refresh_token.into(),
            })
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    pub async fn logout_all(&self, access: &str) -> ApiResult<()> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/logout-all"))
            .bearer_auth(access)
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    pub async fn forgot_password(&self, email: &str) -> ApiResult<()> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/forgot-password"))
            .json(&ForgotReq {
                email: email.into(),
            })
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    pub async fn reset_password(
        &self,
        email: &str,
        code: &str,
        new_password: &str,
    ) -> ApiResult<()> {
        let res = self
            .inner
            .post(self.url("/api/v1/auth/reset-password"))
            .json(&ResetReq {
                email: email.into(),
                code: code.into(),
                new_password: new_password.into(),
            })
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    // ---------- users ----------
    pub async fn me(&self, access: &str) -> ApiResult<UserDto> {
        let res = self
            .inner
            .get(self.url("/api/v1/users/me"))
            .bearer_auth(access)
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn update_me(
        &self,
        access: &str,
        username: Option<&str>,
        display_name: Option<&str>,
        status: Option<&str>,
        avatar_key: Option<&str>,
    ) -> ApiResult<UserDto> {
        let res = self
            .inner
            .put(self.url("/api/v1/users/me"))
            .bearer_auth(access)
            .json(&UpdateMeReq {
                username: username.map(|s| s.to_string()),
                display_name: display_name.map(|s| s.to_string()),
                status: status.map(|s| s.to_string()),
                avatar_key: avatar_key.map(|s| s.to_string()),
            })
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn list_users(&self, access: &str, q: &str) -> ApiResult<Vec<UserDto>> {
        let res = self
            .inner
            .get(self.url("/api/v1/users"))
            .bearer_auth(access)
            .query(&[("q", q), ("limit", "20")])
            .send()
            .await?;
        if res.status().is_success() {
            let list: UserList = res.json().await.map_err(BackendError::from)?;
            return Ok(list.users);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    // ---------- rooms ----------
    pub async fn list_rooms(&self, access: &str) -> ApiResult<Vec<RoomDto>> {
        let res = self
            .inner
            .get(self.url("/api/v1/rooms"))
            .bearer_auth(access)
            .send()
            .await?;
        if res.status().is_success() {
            let list: RoomList = res.json().await.map_err(BackendError::from)?;
            return Ok(list.rooms);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn create_room(
        &self,
        access: &str,
        name: Option<&str>,
        is_dm: bool,
        member_ids: Vec<String>,
    ) -> ApiResult<RoomDto> {
        let res = self
            .inner
            .post(self.url("/api/v1/rooms"))
            .bearer_auth(access)
            .json(&CreateRoomReq {
                name: name.map(|s| s.to_string()),
                is_dm: Some(is_dm),
                member_ids: if member_ids.is_empty() {
                    None
                } else {
                    Some(member_ids)
                },
            })
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn join_room(
        &self,
        access: &str,
        room: &str,
        user_id: Option<&str>,
    ) -> ApiResult<()> {
        let res = self
            .inner
            .post(self.url(&format!("/api/v1/rooms/{room}/join")))
            .bearer_auth(access)
            .json(&JoinReq {
                user_id: user_id.map(|s| s.to_string()),
            })
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    pub async fn leave_room(&self, access: &str, room: &str) -> ApiResult<()> {
        let res = self
            .inner
            .post(self.url(&format!("/api/v1/rooms/{room}/leave")))
            .bearer_auth(access)
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Self::err_of(res.status(), res).await)
        }
    }

    // ---------- messages ----------
    pub async fn history(
        &self,
        access: &str,
        room: &str,
        limit: i64,
        before: Option<&str>,
    ) -> ApiResult<Vec<MessageDto>> {
        let mut req = self
            .inner
            .get(self.url(&format!("/api/v1/rooms/{room}/messages")))
            .bearer_auth(access)
            .query(&[("limit", limit.to_string())]);
        if let Some(b) = before {
            req = req.query(&[("before", b)]);
        }
        let res = req.send().await?;
        if res.status().is_success() {
            let list: MessageList = res.json().await.map_err(BackendError::from)?;
            return Ok(list.messages);
        }
        Err(Self::err_of(res.status(), res).await)
    }

    pub async fn post_message(
        &self,
        access: &str,
        room: &str,
        body: &str,
        kind: Option<&str>,
        metadata: Option<serde_json::Value>,
        reply_to: Option<&str>,
    ) -> ApiResult<MessageDto> {
        let res = self
            .inner
            .post(self.url(&format!("/api/v1/rooms/{room}/messages")))
            .bearer_auth(access)
            .json(&PostMessageReq {
                body: body.to_string(),
                kind: kind.map(str::to_string),
                metadata,
                reply_to: reply_to.map(str::to_string),
            })
            .send()
            .await?;
        if res.status().is_success() {
            return res.json().await.map_err(BackendError::from);
        }
        Err(Self::err_of(res.status(), res).await)
    }
}
