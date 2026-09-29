//! Authenticated intake: `POST /register {permit, record}` with auth enabled
//! (the default). Covers the CA-auth strategy end to end against a live
//! 4-node ledger: happy-path commit, then the negative matrix.
//!
//! Matrix: bare record -> 401; forged permit -> 401; foreign CA -> 401;
//! expired -> 401; sub/user_id mismatch -> 403; replay JTI -> 409;
//! duplicate watermark with a FRESH permit -> 200 committed (no burn).
use std::sync::Arc;
use std::time::Duration;

use base64::{Engine, engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}};
use rand_core::OsRng;
use reqwest::StatusCode;
use rustpq::ml_dsa::mldsa65;
use sagex_gateway::{AuthVerifier, LedgerClient, Outbox};
use sagex_gateway::api::AppState;
use sagex_ledger::config::{ConsensusSection, NodeConfig, NodeSection, PeerEntry};
use sagex_ledger::identity::Identity;
use tokio::net::TcpListener;

struct TestCa {
    id: String,
    sk: mldsa65::SecretKey,
    pk: Vec<u8>,
}

impl TestCa {
    fn generate(id: &str) -> Self {
        let (pk, sk) = mldsa65::generate(&mut OsRng);
        Self {
            id: id.into(),
            sk,
            pk: pk.as_bytes().to_vec(),
        }
    }

    fn permit(&self, sub: &str, ttl_secs: u64) -> String {
        sagex_auth::mint(&self.id, sub, ttl_secs, |input| {
            mldsa65::sign(&self.sk, input, b"", &mut OsRng)
                .map(|s| s.as_bytes().to_vec())
                .map_err(|e| format!("{e:?}"))
        })
        .unwrap()
    }

    fn verifier(&self) -> AuthVerifier {
        AuthVerifier::new(true, &self.id, &STANDARD.encode(&self.pk)).unwrap()
    }
}

async fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port()
}

fn record(wm: &str, user: &str) -> serde_json::Value {
    serde_json::json!({
        "watermark": wm,
        "session_id": "sess-auth",
        "timestamp": 1700000000000i64,
        "user_id": user,
        "file_hash": "aa".repeat(32),
        "payload_hash": "bb".repeat(32),
        "auth_server": "auth-test",
        "signature": "c2lnbmF0dXJl"
    })
}

fn envelope(permit: &str, rec: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "permit": permit, "record": rec })
}

fn node_config(
    id: u64,
    addr: &str,
    db: String,
    sk: &str,
    pk: &str,
    peers: Vec<PeerEntry>,
) -> NodeConfig {
    NodeConfig {
        node: NodeSection {
            id,
            listen: addr.into(),
            db,
            secret_key: sk.into(),
            pubkey: pk.into(),
        },
        peers,
        consensus: ConsensusSection { f: 1, view_timeout_ms: 3000 },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn authenticated_register_matrix() {
    let dir = tempfile::tempdir().unwrap();
    let ca = TestCa::generate("sagex-ca");
    let evil = TestCa::generate("evil-ca");

    let ledger_ports = vec![free_port().await, free_port().await, free_port().await, free_port().await];
    let ledger_addrs: Vec<String> =
        ledger_ports.iter().map(|p| format!("127.0.0.1:{p}")).collect();
    let gw_port = free_port().await;
    let gw_base = format!("http://127.0.0.1:{gw_port}");

    let outbox =
        Outbox::open(dir.path().join("gw-auth.db").to_string_lossy().as_ref()).unwrap();
    let ledger = Arc::new(LedgerClient::new(ledger_addrs.clone(), 1500, 2000));
    let state = Arc::new(AppState {
        outbox,
        ledger,
        retry_batch: 16,
        auth: ca.verifier(),
    });
    let app = sagex_gateway::api::router(state.clone());
    let listener = TcpListener::bind(format!("127.0.0.1:{gw_port}")).await.unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // Boot the 4-node ledger.
    let ids: Vec<u64> = vec![0, 1, 2, 3];
    let identities: Vec<Identity> = ids.iter().map(|i| Identity::generate(*i)).collect();
    let mut handles = vec![];
    for (i, id) in ids.iter().enumerate() {
        let mut peers = vec![];
        for (j, pid) in ids.iter().enumerate() {
            if i == j {
                continue;
            }
            peers.push(PeerEntry {
                id: *pid,
                addr: ledger_addrs[j].clone(),
                pubkey: identities[j].public_b64.clone(),
            });
        }
        let cfg = node_config(
            *id,
            &ledger_addrs[i],
            dir.path().join(format!("auth-n{id}.db")).to_string_lossy().into_owned(),
            &identities[i].secret_b64(),
            &identities[i].public_b64,
            peers,
        );
        let node = sagex_ledger::node::Node::new(cfg).unwrap();
        handles.push(tokio::spawn(async move {
            let _ = node.run().await;
        }));
    }
    tokio::time::sleep(Duration::from_secs(2)).await;

    let http = reqwest::Client::new();
    let post = |body: serde_json::Value| {
        let http = http.clone();
        let gw_base = gw_base.clone();
        async move {
            http.post(format!("{gw_base}/register"))
                .json(&body)
                .timeout(Duration::from_secs(30))
                .send()
                .await
                .unwrap()
        }
    };

    // Happy path: valid permit + matching user_id -> 201 committed.
    let p_alice = ca.permit("alice", 3600);
    let r = post(envelope(&p_alice, record("wm-auth-1", "alice"))).await;
    assert_eq!(r.status(), StatusCode::CREATED, "happy path");
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["status"], "committed");
    assert_eq!(body["block_index"], 0);

    // Replay: same permit again (fresh watermark) -> 409, nothing stored.
    let r = post(envelope(&p_alice, record("wm-auth-2", "alice"))).await;
    assert_eq!(r.status(), StatusCode::CONFLICT, "replay");
    let ob: serde_json::Value = http
        .get(format!("{gw_base}/outbox"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        ob.as_array().unwrap().iter().all(|e| e["watermark"] != "wm-auth-2"),
        "replayed permit must not store"
    );

    // Bare record with auth enabled -> 401.
    let r = post(record("wm-auth-3", "alice")).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "bare record");

    // Forged permit (re-signed claims) -> 401.
    let forged = {
        let parts: Vec<&str> = p_alice.split('.').collect();
        let mut claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        claims["sub"] = serde_json::Value::String("mallory".into());
        let p2 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        format!("{}.{}.{}", parts[0], p2, parts[2])
    };
    let r = post(envelope(&forged, record("wm-auth-4", "mallory"))).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "forged permit");

    // Foreign CA permit -> 401.
    let r = post(envelope(
        &evil.permit("alice", 3600),
        record("wm-auth-5", "alice"),
    ))
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "foreign CA");

    // Expired permit -> 401. Mint with TTL 0, wait out the second boundary,
    // then submit the SAME permit: exp < now is now guaranteed.
    let p_expired = ca.permit("alice", 0);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let r = post(envelope(
        &p_expired,
        record("wm-auth-6", "alice"),
    ))
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "expired permit");

    // sub/user_id mismatch -> 403.
    let r = post(envelope(
        &ca.permit("carol", 3600),
        record("wm-auth-7", "dave"),
    ))
    .await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN, "sub mismatch");

    // Duplicate watermark with a FRESH permit -> 200 committed-duplicate,
    // and the fresh permit must NOT be burned (still usable afterwards).
    let p_fresh = ca.permit("alice", 3600);
    let r = post(envelope(&p_fresh, record("wm-auth-1", "alice"))).await;
    assert_eq!(r.status(), StatusCode::OK, "duplicate short-circuit");
    let dup: serde_json::Value = r.json().await.unwrap();
    assert_eq!(dup["duplicate"], true);
    let r = post(envelope(&p_fresh, record("wm-auth-8", "alice"))).await;
    assert_eq!(r.status(), StatusCode::CREATED, "fresh permit not burned by dup");

    for h in handles {
        h.abort();
    }
}
