//! End-to-end: permits issuance, CSR triple bind, key directory — against
//! a live CA (temp dir + scratch Mongo db) with hand-minted chatsrv JOSE
//! tokens and RG tokens (same shapes as the real issuers).
//!
//! Requires MongoDB at `TEST_MONGODB_URI`
//! (default `mongodb://sagexca:change-me-in-dot-env@127.0.0.1:27017/?authSource=admin`).
use std::{sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}};
use rand_core::{OsRng, RngCore};
use reqwest::StatusCode;
use sagex_certauth::{app, ca::CaMaterial};
use serde_json::{json, Value};

const CHATSRV_CTX: &[u8] = b"sagex-chatsrv/jose-mldsa65-v1";
const CHAT_KID: &str = "test-chat-kid";
const RG_ID: &str = "test-rg-01";

fn mongo_uri() -> String {
    std::env::var("TEST_MONGODB_URI").unwrap_or_else(|_| {
        "mongodb://sagexca:change-me-in-dot-env@127.0.0.1:27017/?authSource=admin".to_string()
    })
}

struct Keys {
    sk: rustpq::ml_dsa::mldsa65::SecretKey,
    pk: Vec<u8>,
}

impl Keys {
    fn generate() -> Self {
        let (pk, sk) = rustpq::ml_dsa::mldsa65::generate(&mut OsRng);
        Self {
            sk,
            pk: pk.as_bytes().to_vec(),
        }
    }

    fn rg_token(&self, ttl_secs: u64) -> String {
        sagex_auth::rg_token::mint(RG_ID, ttl_secs, |input| {
            rustpq::ml_dsa::mldsa65::sign(&self.sk, input, b"", &mut OsRng)
                .map(|s| s.as_bytes().to_vec())
                .map_err(|e| format!("{e:?}"))
        })
        .unwrap()
    }

    /// Chatsrv-shaped JOSE access token (mirrors chatsrv `issue()`).
    fn chatsrv_token(&self, username: Option<&str>, purpose: &str, ttl: i64) -> String {
        let now = sagex_auth::now_secs() as i64;
        let header = json!({"alg": "ML-DSA-65", "typ": "JWT", "kid": CHAT_KID});
        let mut payload = json!({
            "sub": "0123456789abcdef01234567",
            "iat": now, "exp": now + ttl,
            "jti": "test-jti", "purpose": purpose,
        });
        if let Some(u) = username {
            payload["username"] = Value::String(u.into());
        }
        let p1 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
        let p2 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        let input = format!("{p1}.{p2}");
        let sig =
            rustpq::ml_dsa::mldsa65::sign(&self.sk, input.as_bytes(), CHATSRV_CTX, &mut OsRng)
                .unwrap();
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.as_bytes()))
    }
}

struct World {
    base: String,
    chat: Keys,
    rg: Keys,
}

async fn boot(strict: bool) -> World {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "sagex-ca-e2e-{}-{}-{}",
        std::process::id(),
        nanos,
        if strict { "strict" } else { "lax" }
    ));
    let (ca, _) = CaMaterial::load_or_generate(&dir, b"test-password", "test-ca").unwrap();
    let signer = Arc::new(ca.signer(b"test-password").unwrap());
    let chat = Keys::generate();
    let rg = Keys::generate();
    let client = mongodb::Client::with_uri_str(&mongo_uri()).await.unwrap();
    let state = app::AppState {
        client,
        db_name: format!("sagexcatest_{nanos}_{}", if strict { "s" } else { "l" }),
        ca_identity: ca.identity.clone(),
        ca_dsa_public: ca.dsa_public.clone(),
        signer,
        validity_days: 30,
        chatsrv_dsa_public: chat.pk.clone(),
        chatsrv_kid: CHAT_KID.into(),
        rg_dsa_public: rg.pk.clone(),
        rg_id: RG_ID.into(),
        require_chatsrv_token: strict,
        permit_ttl_secs: 3600,
    };
    app::ensure_indexes(&state).await.unwrap();
    let app_router = app::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app_router).await;
    });
    World { base, chat, rg }
}

impl World {
    fn http(&self) -> reqwest::Client {
        reqwest::Client::new()
    }

    /// Fresh client keypair + PoP-signed CSR body (mirrors mkcsr).
    fn csr_body(&self, identity: &str) -> Value {
        let mut pw = [0u8; 16];
        OsRng.fill_bytes(&mut pw);
        let (_ke, kpk) =
            sagex_crypto::pqc::kem::KeyGen::generate_from_password(&pw).unwrap();
        let (de, dpk) =
            sagex_crypto::pqc::dsa::KeyGen::generate_from_password(&pw).unwrap();
        let msg = sagex_certauth::csr::pop_message(identity, &kpk, &dpk);
        let sig =
            sagex_crypto::pqc::dsa::Implement::sign_from_password(de, &pw, &msg, b"").unwrap();
        json!({
            "identity": identity,
            "key_kem_b64": STANDARD.encode(&kpk),
            "key_dsa_b64": STANDARD.encode(&dpk),
            "self_sig_b64": STANDARD.encode(sig.as_bytes()),
        })
    }

    async fn enroll(&self, username: &str, token: &str) -> (u16, Value) {
        let permit: Value = self
            .http()
            .post(format!("{}/v1/permits", self.base))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let r = self
            .http()
            .post(format!("{}/v1/csr", self.base))
            .json(&json!({
                "permit": permit["permit"],
                "csr": self.csr_body(username),
                "chatsrv_token": token,
            }))
            .send()
            .await
            .unwrap();
        let code = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        (code, v)
    }
}

#[tokio::test]
async fn permits_endpoint_matrix() {
    let w = boot(false).await;
    let good = w.chat.chatsrv_token(Some("alice"), "access", 900);

    // No token -> 401.
    let r = w
        .http()
        .post(format!("{}/v1/permits", w.base))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // Garbage -> 401.
    let r = w
        .http()
        .post(format!("{}/v1/permits", w.base))
        .bearer_auth("garbage.token.here")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // Refresh-purpose token at an access gate -> 401.
    let r = w
        .http()
        .post(format!("{}/v1/permits", w.base))
        .bearer_auth(w.chat.chatsrv_token(Some("alice"), "refresh", 900))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // Foreign chatsrv key -> 401.
    let evil = Keys::generate();
    let evil_tok = {
        let now = sagex_auth::now_secs() as i64;
        let h = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({"alg":"ML-DSA-65","typ":"JWT","kid":CHAT_KID})).unwrap(),
        );
        let p = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({"sub":"x","username":"mallory","iat":now,"exp":now+900,"jti":"j","purpose":"access"})).unwrap(),
        );
        let input = format!("{h}.{p}");
        let sk = &evil.sk;
        let sig = rustpq::ml_dsa::mldsa65::sign(sk, input.as_bytes(), CHATSRV_CTX, &mut OsRng).unwrap();
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.as_bytes()))
    };
    let r = w
        .http()
        .post(format!("{}/v1/permits", w.base))
        .bearer_auth(&evil_tok)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // Happy path -> 201 kind=user bound to token username.
    let r = w
        .http()
        .post(format!("{}/v1/permits", w.base))
        .bearer_auth(&good)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["sub"], "alice");
    assert!(!v["permit"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn csr_triple_bind_matrix() {
    let w = boot(false).await;
    let alice_tok = w.chat.chatsrv_token(Some("alice"), "access", 900);

    // Happy path with triple bind -> 201.
    let (code, cert) = w.enroll("alice", &alice_tok).await;
    assert_eq!(code, 201, "{cert}");
    assert_eq!(cert["identity"], "alice");

    // Mismatch: bob's token, alice's permit+CSR -> 403.
    let bob_tok = w.chat.chatsrv_token(Some("bob"), "access", 900);
    let permit: Value = w
        .http()
        .post(format!("{}/v1/permits", w.base))
        .bearer_auth(&alice_tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let r = w
        .http()
        .post(format!("{}/v1/csr", w.base))
        .json(&json!({
            "permit": permit["permit"],
            "csr": w.csr_body("alice"),
            "chatsrv_token": bob_tok,
        }))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    // Legacy path (no token, non-strict) still works for a fresh identity.
    let permit2: Value = w
        .http()
        .post(format!("{}/v1/permits", w.base))
        .bearer_auth(&w.chat.chatsrv_token(Some("carol"), "access", 900))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let r = w
        .http()
        .post(format!("{}/v1/csr", w.base))
        .json(&json!({
            "permit": permit2["permit"],
            "csr": w.csr_body("carol"),
        }))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn csr_strict_mode() {
    let w = boot(true).await;
    let tok = w.chat.chatsrv_token(Some("dave"), "access", 900);
    let permit: Value = w
        .http()
        .post(format!("{}/v1/permits", w.base))
        .bearer_auth(&tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // No token in strict mode -> 401.
    let r = w
        .http()
        .post(format!("{}/v1/csr", w.base))
        .json(&json!({
            "permit": permit["permit"],
            "csr": w.csr_body("dave"),
        }))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    // With token -> 201.
    let (code, _) = w.enroll("dave", &tok).await;
    assert_eq!(code, 201);
}

#[tokio::test]
async fn keys_directory_matrix() {
    let w = boot(false).await;
    let tok = w.chat.chatsrv_token(Some("erin"), "access", 900);
    let (code, _) = w.enroll("erin", &tok).await;
    assert_eq!(code, 201);

    let rg_tok = w.rg.rg_token(60);
    let http = w.http();

    // No token -> 401.
    let r = http
        .get(format!("{}/v1/keys?identity=erin", w.base))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // Batch: known + unknown.
    let r = http
        .get(format!("{}/v1/keys?identity=erin,ghost", w.base))
        .bearer_auth(&rg_tok)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["keys"].as_array().unwrap().len(), 1);
    assert_eq!(v["keys"][0]["identity"], "erin");
    assert!(!v["keys"][0]["key_dsa_b64"].as_str().unwrap().is_empty());
    assert!(!v["keys"][0]["fingerprint"].as_str().unwrap().is_empty());
    assert_eq!(v["errors"][0]["identity"], "ghost");

    // Single lookup.
    let r = http
        .get(format!("{}/v1/keys/erin", w.base))
        .bearer_auth(&w.rg.rg_token(60))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Foreign RG -> 401; expired RG -> 401.
    let evil = Keys::generate();
    let evil_tok = sagex_auth::rg_token::mint("other-rg", 60, |input| {
        rustpq::ml_dsa::mldsa65::sign(&evil.sk, input, b"", &mut OsRng)
            .map(|s| s.as_bytes().to_vec())
            .map_err(|e| format!("{e:?}"))
    })
    .unwrap();
    let r = http
        .get(format!("{}/v1/keys?identity=erin", w.base))
        .bearer_auth(&evil_tok)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // Expired RG token -> 401.
    let stale = w.rg.rg_token(0);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let r = http
        .get(format!("{}/v1/keys?identity=erin", w.base))
        .bearer_auth(&stale)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
