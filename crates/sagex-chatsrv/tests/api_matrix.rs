//! Full-surface integration tests for the rich-chat API (all 26 routes).
//! Boots the real router against a scratch Mongo database with
//! [`FakeStorage`] (no MinIO needed) and ephemeral ML-DSA keys.
//!
//! Requires MongoDB at `TEST_MONGODB_URI`
//! (default `mongodb://sagexca:change-me-in-dot-env@127.0.0.1:27017/?authSource=admin`).
use std::sync::Arc;

use mongodb::bson::DateTime;
use sagex_chatsrv::{
    jose_mldsa::JoseSigner,
    models::User,
    routes,
    state::{ensure_indexes, AppState},
    storage::FakeStorage,
    ws::ChatHub,
};
use serde_json::{json, Value};

fn mongo_uri() -> String {
    std::env::var("TEST_MONGODB_URI").unwrap_or_else(|_| {
        "mongodb://sagexca:change-me-in-dot-env@127.0.0.1:27017/?authSource=admin".to_string()
    })
}

struct World {
    base: String,
    jose: Arc<JoseSigner>,
    db_name: String,
}

async fn boot() -> World {
    let db_name = format!(
        "chat_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let client = mongodb::Client::with_uri_str(&mongo_uri()).await.unwrap();
    let db = client.database(&db_name);
    ensure_indexes(&db).await.unwrap();
    let (jose, _) =
        JoseSigner::from_env_or_ephemeral(None, None, "test-kid".into(), 900, 2_592_000)
            .unwrap();
    let jose = Arc::new(jose);
    let state = AppState {
        db: db.clone(),
        jose: jose.clone(),
        hub: Arc::new(ChatHub::new()),
        config: Arc::new(sagex_chatsrv::config::Config {
            mongodb_uri: mongo_uri(),
            db_name: db_name.clone(),
            bind_addr: "127.0.0.1:0".into(),
            access_token_ttl_secs: 900,
            refresh_token_ttl_secs: 2_592_000,
            otp_ttl_secs: 600,
            otp_resend_cooldown_secs: 60,
            otp_max_attempts: 5,
            mldsa65_sk_b64: None,
            mldsa65_pk_b64: None,
            mldsa65_kid: "test-kid".into(),
            minio_endpoint: String::new(),
            minio_bucket: "test-bucket".into(),
            minio_access_key: String::new(),
            minio_secret_key: String::new(),
            media_max_bytes: 1024 * 1024,
            presign_ttl_secs: 300,
        }),
        storage: Arc::new(FakeStorage::default()),
    };
    let app = routes::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    World { base, jose, db_name }
}

impl World {
    async fn user(&self, username: &str) -> (String, String) {
        // Insert a verified user directly, mint an access token.
        let client = mongodb::Client::with_uri_str(&mongo_uri()).await.unwrap();
        let users = client
            .database(&self.db_name)
            .collection::<User>("users");
        let now = DateTime::now();
        let res = users
            .insert_one(
                User {
                    id: None,
                    email: format!("{username}@t.co"),
                    username: username.into(),
                    display_name: None,
                    status: None,
                    avatar_key: None,
                    password_hash: "x".into(),
                    is_verified: true,
                    created_at: now,
                    updated_at: now,
                },
                None,
            )
            .await
            .unwrap();
        let hex = res.inserted_id.as_object_id().unwrap().to_hex();
        let (token, _, _) = self.jose.issue(&hex, "access").unwrap();
        (hex, token)
    }

    fn http(&self) -> reqwest::Client {
        reqwest::Client::new()
    }

    async fn post(&self, token: &str, path: &str, body: Value) -> (u16, Value) {
        let r = self
            .http()
            .post(format!("{}{}", self.base, path))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let code = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        (code, v)
    }

    async fn get(&self, token: &str, path: &str) -> (u16, Value) {
        let r = self
            .http()
            .get(format!("{}{}", self.base, path))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        let code = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        (code, v)
    }

    async fn patch(&self, token: &str, path: &str, body: Value) -> (u16, Value) {
        let r = self
            .http()
            .patch(format!("{}{}", self.base, path))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let code = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        (code, v)
    }

    async fn delete(&self, token: &str, path: &str) -> (u16, Value) {
        let r = self
            .http()
            .delete(format!("{}{}", self.base, path))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        let code = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        (code, v)
    }

    async fn delete_q(&self, token: &str, path: &str, query: &[(&str, &str)]) -> (u16, Value) {
        let r = self
            .http()
            .delete(format!("{}{}", self.base, path))
            .query(query)
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        let code = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        (code, v)
    }

    async fn put(&self, token: &str, path: &str, body: Value) -> (u16, Value) {
        let r = self
            .http()
            .put(format!("{}{}", self.base, path))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let code = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        (code, v)
    }

    async fn room(&self, token: &str, members: Vec<String>) -> String {
        let (code, v) = self
            .post(
                token,
                "/api/v1/rooms",
                json!({ "name": "t", "member_ids": members }),
            )
            .await;
        assert_eq!(code, 201, "create room: {v}");
        v["id"].as_str().unwrap().to_string()
    }
}

#[tokio::test]
async fn media_presign_matrix() {
    let w = boot().await;
    let (_, tok) = w.user("meda").await;
    let (_, stranger) = w.user("medb").await;
    let room = w.room(&tok, vec![]).await;

    // Bad content type.
    let (c, _) = w
        .post(
            &tok,
            "/api/v1/media/presign",
            json!({ "room_id": room, "filename": "x.exe", "content_type": "application/x-msdownload", "size_bytes": 10 }),
        )
        .await;
    assert_eq!(c, 400);

    // Oversize.
    let (c, _) = w
        .post(
            &tok,
            "/api/v1/media/presign",
            json!({ "room_id": room, "filename": "big.png", "content_type": "image/png", "size_bytes": 1 << 30 }),
        )
        .await;
    assert_eq!(c, 400);

    // Non-member.
    let (c, _) = w
        .post(
            &stranger,
            "/api/v1/media/presign",
            json!({ "room_id": room, "filename": "a.png", "content_type": "image/png", "size_bytes": 10 }),
        )
        .await;
    assert_eq!(c, 403);

    // Happy path (fake:// URL — registered in FakeStorage).
    let (c, v) = w
        .post(
            &tok,
            "/api/v1/media/presign",
            json!({ "room_id": room, "filename": "a b.png", "content_type": "image/png", "size_bytes": 10 }),
        )
        .await;
    assert_eq!(c, 200, "{v}");
    let key = v["object_key"].as_str().unwrap();
    assert!(key.starts_with(&format!("media/{room}/")));
    assert!(!key.contains(' '));

    // Download: malformed room -> 400; valid room + unknown key -> 404.
    let (c, _) = w.get(&tok, "/api/v1/media/media/nope/x").await;
    assert_eq!(c, 400);
    let (c, _) = w.get(&tok, &format!("/api/v1/media/media/{room}/nope")).await;
    assert_eq!(c, 404);
    let (c, v) = w
        .get(&tok, &format!("/api/v1/media/{key}"))
        .await;
    assert_eq!(c, 200, "{v}");
    assert!(v["get_url"].as_str().unwrap().starts_with("fake://get/"));

    // Stranger cannot download room media.
    let (c, _) = w.get(&stranger, &format!("/api/v1/media/{key}")).await;
    assert_eq!(c, 403);
}

#[tokio::test]
async fn rich_post_and_poll_flow() {
    let w = boot().await;
    let (_, tok) = w.user("pola").await;
    let room = w.room(&tok, vec![]).await;

    // Unknown kind rejected.
    let (c, _) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "x", "kind": "teleport" }),
        )
        .await;
    assert_eq!(c, 400);

    // Image without object_key rejected.
    let (c, _) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "pic", "kind": "image", "metadata": {} }),
        )
        .await;
    assert_eq!(c, 400);

    // Register an object via presign, then post an image + reply chain.
    let (_, pv) = w
        .post(
            &tok,
            "/api/v1/media/presign",
            json!({ "room_id": room, "filename": "p.png", "content_type": "image/png", "size_bytes": 5 }),
        )
        .await;
    let key = pv["object_key"].as_str().unwrap().to_string();
    let (c, img) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "pic", "kind": "image", "metadata": { "object_key": key, "caption": "hi" } }),
        )
        .await;
    assert_eq!(c, 201, "{img}");
    assert_eq!(img["kind"], "image");
    let img_id = img["id"].as_str().unwrap().to_string();

    // Reply to it.
    let (c, rep) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "nice", "reply_to": img_id }),
        )
        .await;
    assert_eq!(c, 201, "{rep}");
    assert_eq!(rep["reply_to"], img_id);

    // Reply to unknown message -> 404.
    let (c, _) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "x", "reply_to": "000000000000000000000000" }),
        )
        .await;
    assert_eq!(c, 404);

    // Poll with options, then vote matrix.
    let (c, poll) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "vote", "kind": "poll", "metadata": { "question": "q?", "options": ["a", "b"] } }),
        )
        .await;
    assert_eq!(c, 201, "{poll}");
    let pid = poll["id"].as_str().unwrap().to_string();

    // Poll without metadata rejected.
    let (c, _) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "vote", "kind": "poll" }),
        )
        .await;
    assert_eq!(c, 400);

    let vote_url = format!("/api/v1/rooms/{room}/messages/{pid}/vote");
    let (c, _v) = w.post(&tok, &vote_url, json!({ "option_idx": 5 })).await;
    assert_eq!(c, 400, "option out of range");
    let (c, v) = w.post(&tok, &vote_url, json!({ "option_idx": 1 })).await;
    assert_eq!(c, 200, "{v}");
    assert_eq!(v["tallies"], json!([0, 1]));
    // Change vote replaces.
    let (c, v) = w.post(&tok, &vote_url, json!({ "option_idx": 0 })).await;
    assert_eq!(c, 200, "{v}");
    assert_eq!(v["tallies"], json!([1, 0]));
    // History shows tallies + my vote.
    let (c, h) = w.get(&tok, &format!("/api/v1/rooms/{room}/messages")).await;
    assert_eq!(c, 200);
    let found = h["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == pid)
        .unwrap();
    assert_eq!(found["poll"]["total"], 1);
    assert_eq!(found["poll"]["my_vote"], 0);
    // Retract.
    let (c, _) = w.delete(&tok, &vote_url).await;
    assert_eq!(c, 200);
    let (c, _) = w.delete(&tok, &vote_url).await;
    assert_eq!(c, 404, "double retract");

    // Voting on a text message -> 400.
    let (c, t) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "plain" }),
        )
        .await;
    assert_eq!(c, 201);
    let tid = t["id"].as_str().unwrap();
    let (c, _) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages/{tid}/vote"),
            json!({ "option_idx": 0 }),
        )
        .await;
    assert_eq!(c, 400);
}

#[tokio::test]
async fn reactions_pins_stars_read_edit_delete() {
    let w = boot().await;
    let (_, tok) = w.user("reaa").await;
    let room = w.room(&tok, vec![]).await;
    let (_, m) = w
        .post(
            &tok,
            &format!("/api/v1/rooms/{room}/messages"),
            json!({ "body": "hello" }),
        )
        .await;
    let mid = m["id"].as_str().unwrap().to_string();
    let rbase = format!("/api/v1/rooms/{room}/messages/{mid}");

    // Reactions: add, dup 409, history tally, remove, missing 404.
    let (c, _) = w
        .post(&tok, &format!("{rbase}/reactions"), json!({ "emoji": "👍" }))
        .await;
    assert_eq!(c, 201);
    let (c, _) = w
        .post(&tok, &format!("{rbase}/reactions"), json!({ "emoji": "👍" }))
        .await;
    assert_eq!(c, 409);
    let (_, h) = w.get(&tok, &format!("/api/v1/rooms/{room}/messages")).await;
    let found = h["messages"].as_array().unwrap().iter().find(|m| m["id"] == mid).unwrap();
    assert_eq!(found["reactions"][0]["emoji"], "👍");
    assert_eq!(found["reactions"][0]["mine"], true);
    let (c, _) = w
        .delete_q(&tok, &format!("{rbase}/reactions"), &[("emoji", "👍")])
        .await;
    assert_eq!(c, 200);
    let (c, _) = w
        .delete_q(&tok, &format!("{rbase}/reactions"), &[("emoji", "👍")])
        .await;
    assert_eq!(c, 404);

    // Pins: add, list embeds message, unpin, missing 404.
    let (c, _) = w
        .post(&tok, &format!("/api/v1/rooms/{room}/pins"), json!({ "message_id": mid }))
        .await;
    assert_eq!(c, 201);
    let (c, p) = w.get(&tok, &format!("/api/v1/rooms/{room}/pins")).await;
    assert_eq!(c, 200, "{p}");
    assert_eq!(p["pins"][0]["message"]["id"], mid);
    let (c, h) = w.get(&tok, &format!("/api/v1/rooms/{room}/messages")).await;
    assert_eq!(h["messages"][0]["pinned"], true);
    let (c, _) = w.delete(&tok, &format!("/api/v1/rooms/{room}/pins/{mid}")).await;
    assert_eq!(c, 200);

    // Stars: add, list, unstar.
    let (c, _) = w.post(&tok, "/api/v1/users/me/starred", json!({ "message_id": mid })).await;
    assert_eq!(c, 201);
    let (c, s) = w.get(&tok, "/api/v1/users/me/starred").await;
    assert_eq!(c, 200, "{s}");
    assert_eq!(s["messages"][0]["id"], mid);
    let (c, _) = w.delete(&tok, &format!("/api/v1/users/me/starred/{mid}")).await;
    assert_eq!(c, 200);

    // Read watermark.
    let (c, _) = w
        .post(&tok, &format!("/api/v1/rooms/{room}/read"), json!({ "message_id": mid }))
        .await;
    assert_eq!(c, 200);

    // Edit by other forbidden; edit ok; edit deleted fails.
    let (_, tok2) = w.user("reab").await;
    // tok2 is not a member at all -> 403 via membership first.
    let (c, _) = w
        .patch(&tok2, &rbase, json!({ "body": "hijack" }))
        .await;
    assert!(c == 403 || c == 404, "non-member blocked: {c}");
    let (c, _) = w.patch(&tok, &rbase, json!({ "body": "  " })).await;
    assert_eq!(c, 400, "empty edit");
    let (c, _) = w.patch(&tok, &rbase, json!({ "body": "edited!" })).await;
    assert_eq!(c, 200);
    let (c, _) = w.delete(&tok, &rbase).await;
    assert_eq!(c, 200);
    let (c, h) = w.get(&tok, &format!("/api/v1/rooms/{room}/messages")).await;
    assert_eq!(c, 200);
    let found = h["messages"].as_array().unwrap().iter().find(|m| m["id"] == mid).unwrap();
    assert_eq!(found["deleted"], true);
    assert_eq!(found["body"], "");
    let (c, _) = w.patch(&tok, &rbase, json!({ "body": "again" })).await;
    assert_eq!(c, 400, "edit deleted");
}

#[tokio::test]
async fn rooms_search_settings_membership_export() {
    let w = boot().await;
    let (uid_a, tok_a) = w.user("rooma").await;
    let (uid_b, tok_b) = w.user("roomb").await;
    let room = w.room(&tok_a, vec![uid_b.clone()]).await;

    // Search.
    w.post(&tok_a, &format!("/api/v1/rooms/{room}/messages"), json!({ "body": "hello world picnic" })).await;
    let (c, s) = w.get(&tok_a, &format!("/api/v1/rooms/{room}/messages/search?q=picnic")).await;
    assert_eq!(c, 200, "{s}");
    assert_eq!(s["messages"].as_array().unwrap().len(), 1);
    let (c, _) = w.get(&tok_a, &format!("/api/v1/rooms/{room}/messages/search?q=")).await;
    assert_eq!(c, 400);

    // Settings roundtrip + bad datetime.
    let (_, g) = w.get(&tok_a, &format!("/api/v1/users/me/rooms/{room}/settings")).await;
    assert_eq!(g["archived"], false);
    let (c, _) = w
        .put(&tok_a, &format!("/api/v1/users/me/rooms/{room}/settings"), json!({ "archived": true, "fav": true }))
        .await;
    assert_eq!(c, 200);
    let (c, g) = w.get(&tok_a, &format!("/api/v1/users/me/rooms/{room}/settings")).await;
    assert_eq!(g["archived"], true);
    let (c, _) = w
        .put(&tok_a, &format!("/api/v1/users/me/rooms/{room}/settings"), json!({ "muted_until": "not-a-date" }))
        .await;
    assert_eq!(c, 400);

    // Remove member: self-remove rejected; stranger e2e.
    let (c, _) = w
        .post(&tok_a, &format!("/api/v1/rooms/{room}/remove"), json!({ "user_id": uid_a }))
        .await;
    assert_eq!(c, 400);
    let (c, _) = w
        .post(&tok_a, &format!("/api/v1/rooms/{room}/remove"), json!({ "user_id": uid_b }))
        .await;
    assert_eq!(c, 200);
    let (c, _) = w
        .post(&tok_b, &format!("/api/v1/rooms/{room}/messages"), json!({ "body": "im out" }))
        .await;
    assert_eq!(c, 403, "removed member blocked");

    // Export contains history.
    let (c, e) = w.get(&tok_a, &format!("/api/v1/rooms/{room}/export")).await;
    assert_eq!(c, 200, "{e}");
    assert!(e["message_count"].as_u64().unwrap() >= 1);

    // User lookup.
    let (c, u) = w.get(&tok_a, &format!("/api/v1/users/{uid_b}")).await;
    assert_eq!(c, 200, "{u}");
    assert_eq!(u["username"], "roomb");
    let (c, _) = w.get(&tok_a, "/api/v1/users/000000000000000000000000").await;
    assert_eq!(c, 404);
}

#[tokio::test]
async fn dm_lookup_report_block() {
    let w = boot().await;
    let (_, tok_a) = w.user("dmaa").await;
    let (uid_b, tok_b) = w.user("dmbb").await;

    // Lookup creates, second call finds.
    let (c, d1) = w.post(&tok_a, "/api/v1/dms/lookup", json!({ "user_id": uid_b })).await;
    assert_eq!(c, 201, "{d1}");
    let (c, d2) = w.post(&tok_a, "/api/v1/dms/lookup", json!({ "user_id": uid_b })).await;
    assert_eq!(c, 200, "{d2}");
    assert_eq!(d1["room_id"], d2["room_id"]);
    // Self-DM rejected.
    let (uid_a2, tok_a2) = w.user("dmac").await;
    let (c, _) = w.post(&tok_a2, "/api/v1/dms/lookup", json!({ "user_id": uid_a2 })).await;
    assert_eq!(c, 400);

    // Report: create, dup 409.
    let (c, _) = w.post(&tok_a, &format!("/api/v1/users/{uid_b}/report"), json!({ "reason": "spam" })).await;
    assert_eq!(c, 201);
    let (c, _) = w.post(&tok_a, &format!("/api/v1/users/{uid_b}/report"), json!({})).await;
    assert_eq!(c, 409);

    // Block: create, dup 409, DM blocked both directions, unblock restores.
    let (c, _) = w.post(&tok_a, &format!("/api/v1/users/{uid_b}/blocks"), json!({})).await;
    assert_eq!(c, 201);
    let (c, _) = w.post(&tok_a, &format!("/api/v1/users/{uid_b}/blocks"), json!({})).await;
    assert_eq!(c, 409);
    let (c, _) = w.post(&tok_b, "/api/v1/dms/lookup", json!({ "user_id": d1["room_id"].as_str().unwrap_or("x") })).await;
    assert!(c == 400 || c == 404 || c == 403, "garbage id rejected: {c}");
    // tok_b tries DM with tok_a's user: need a's id — resolve via users list.
    let (c, ul) = w.get(&tok_b, "/api/v1/users?q=dmaa").await;
    assert_eq!(c, 200);
    let aid = ul["users"][0]["id"].as_str().unwrap().to_string();
    let (c, _) = w.post(&tok_b, "/api/v1/dms/lookup", json!({ "user_id": aid })).await;
    assert_eq!(c, 403, "blocked direction");
    let (c, _) = w.delete(&tok_a, &format!("/api/v1/users/{uid_b}/blocks")).await;
    assert_eq!(c, 200);
    let (c, d3) = w.post(&tok_b, "/api/v1/dms/lookup", json!({ "user_id": aid })).await;
    assert_eq!(c, 200, "unblocked restores: {d3}");
}

#[tokio::test]
async fn profile_status_avatar() {
    let w = boot().await;
    let (_, tok) = w.user("profa").await;
    let room = w.room(&tok, vec![]).await;

    // Status update roundtrips through me().
    let (c, _) = w.put(&tok, "/api/v1/users/me", json!({ "status": "Out for lunch" })).await;
    assert_eq!(c, 200);
    let (c, me) = w.get(&tok, "/api/v1/users/me").await;
    assert_eq!(c, 200);
    assert_eq!(me["status"], "Out for lunch");
    // Overlong status rejected.
    let (c, _) = w.put(&tok, "/api/v1/users/me", json!({ "status": "x".repeat(200) })).await;
    assert_eq!(c, 400);

    // Avatar must reference an uploaded object.
    let (c, _) = w.put(&tok, "/api/v1/users/me", json!({ "avatar_key": "media/elsewhere/x" })).await;
    assert_eq!(c, 400);
    let (_, pv) = w
        .post(&tok, "/api/v1/media/presign", json!({ "room_id": room, "filename": "av.png", "content_type": "image/png", "size_bytes": 8 }))
        .await;
    let key = pv["object_key"].as_str().unwrap().to_string();
    let (c, me) = w.put(&tok, "/api/v1/users/me", json!({ "avatar_key": key })).await;
    assert_eq!(c, 200, "{me}");
    assert_eq!(me["avatar_key"], key);
}
