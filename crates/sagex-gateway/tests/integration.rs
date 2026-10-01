//! End-to-end: gateway HTTP intake + outbox durability + ledger proxy.
//! Phase 1: ledger down -> POST returns 202 queued, outbox holds the row.
//! Phase 2: ledger boots -> worker drains -> record committed + queryable.
//! Phase 3: live submit, duplicate resubmit, user query, 1 dead follower.
use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use sagex_gateway::{AuthVerifier, LedgerClient, Outbox};
use sagex_gateway::api::AppState;
use sagex_ledger::config::{ConsensusSection, NodeConfig, NodeSection, PeerEntry};
use sagex_ledger::identity::Identity;
use tokio::net::TcpListener;

async fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port()
}

fn record(wm: &str, user: &str) -> serde_json::Value {
    serde_json::json!({
        "watermark": wm,
        "session_id": "sess-gw",
        "timestamp": 1700000000000i64,
        "user_id": user,
        "file_hash": "aa".repeat(32),
        "payload_hash": "bb".repeat(32),
        "auth_server": "auth-test",
        "signature": "c2lnbmF0dXJl"
    })
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

async fn poll_record(
    http: &reqwest::Client,
    base: &str,
    wm: &str,
) -> serde_json::Value {
    for _ in 0..60 {
        let r = http.get(format!("{base}/record/{wm}")).send().await.unwrap();
        if r.status() == StatusCode::OK {
            let v: serde_json::Value = r.json().await.unwrap();
            if v["blocks"].as_array().map(|a| !a.is_empty()).unwrap_or(false) {
                return v;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("gateway never served record {wm}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn gateway_queues_when_down_then_drains_and_proxies() {
    let dir = tempfile::tempdir().unwrap();
    let ledger_ports = vec![free_port().await, free_port().await, free_port().await, free_port().await];
    let ledger_addrs: Vec<String> =
        ledger_ports.iter().map(|p| format!("127.0.0.1:{p}")).collect();
    let gw_port = free_port().await;
    let gw_base = format!("http://127.0.0.1:{gw_port}");

    // --- gateway boots FIRST, ledger still down ---
    let outbox =
        Outbox::open(dir.path().join("gw.db").to_string_lossy().as_ref()).unwrap();
    let (_, rg_pub) = outbox.load_or_generate_rg_key().unwrap();
    let rg_fingerprint = sagex_gateway::rg_keys::fingerprint(&rg_pub).unwrap();
    let ledger = Arc::new(LedgerClient::new(ledger_addrs.clone(), 1500, 2000));
    let state = Arc::new(AppState {
        outbox,
        ledger,
        retry_batch: 16,
        // Legacy open-intake coverage: auth deliberately disabled here.
        // Authenticated intake is covered in tests/auth.rs.
        auth: AuthVerifier::disabled(),
        auditor_sub: "ledger-auditor".into(),
        logs_enabled: true,
        rg_fingerprint,
    });
    let app = sagex_gateway::api::router(state.clone());
    tokio::spawn(sagex_gateway::api::outbox_worker(
        state.clone(),
        Duration::from_millis(500),
    ));
    let listener = TcpListener::bind(format!("127.0.0.1:{gw_port}")).await.unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;

    let http = reqwest::Client::new();

    // Phase 1: all nodes down -> 202 queued.
    let r = http
        .post(format!("{gw_base}/register"))
        .json(&record("wm-gw-1", "alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED, "expected 202 queued");
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    let ob: serde_json::Value = http
        .get(format!("{gw_base}/outbox"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ob.as_array().unwrap().len(), 1);

    // --- now boot the 4-node ledger on the reserved ports ---
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
            dir.path().join(format!("n{id}.db")).to_string_lossy().into_owned(),
            &identities[i].secret_b64(),
            &identities[i].public_b64,
            peers,
        );
        let node = sagex_ledger::node::Node::new(cfg).unwrap();
        handles.push(tokio::spawn(async move {
            let _ = node.run().await;
        }));
    }

    // Phase 2: worker drains the queued row once the mesh forms.
    let v = poll_record(&http, &gw_base, "wm-gw-1").await;
    assert_eq!(v["blocks"][0]["record"]["user_id"], "alice");
    assert_eq!(v["blocks"][0]["header"]["index"], 0);

    // Duplicate resubmit -> committed (idempotent, no second block).
    let r = http
        .post(format!("{gw_base}/register"))
        .json(&record("wm-gw-1", "alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let dup: serde_json::Value = r.json().await.unwrap();
    assert_eq!(dup["status"], "committed");
    assert_eq!(dup["duplicate"], true);
    assert_eq!(dup["block_index"], 0);

    // Phase 3: live submit while all nodes up.
    let r = http
        .post(format!("{gw_base}/register"))
        .json(&record("wm-gw-2", "bob"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let live: serde_json::Value = r.json().await.unwrap();
    assert_eq!(live["status"], "committed");
    assert_eq!(live["block_index"], 1);

    // User query proxy.
    let q: serde_json::Value = http
        .get(format!("{gw_base}/records?user_id=alice"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(q["blocks"].as_array().unwrap().len(), 1);

    // Kill one follower; quorum (3/4) must still commit via the gateway.
    handles[3].abort();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let r = http
        .post(format!("{gw_base}/register"))
        .json(&record("wm-gw-3", "carol"))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let v3 = poll_record(&http, &gw_base, "wm-gw-3").await;
    assert_eq!(v3["blocks"][0]["header"]["index"], 2);

    // Status fan-out still reports the survivors.
    let st: serde_json::Value = http
        .get(format!("{gw_base}/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(st["ledger"].as_array().unwrap().len(), 4);

    // Invalid record -> 400, nothing queued.
    let mut bad = record("wm-bad", "x");
    bad["signature"] = serde_json::Value::String(String::new());
    let r = http.post(format!("{gw_base}/register")).json(&bad).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    for h in handles {
        h.abort();
    }
}
