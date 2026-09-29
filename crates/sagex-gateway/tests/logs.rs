//! Restricted auditor logs: `GET /logs` behind a CA-issued auditor permit.
//!
//! Matrix against a live gateway (+ 4-node ledger for fan-out):
//! no token -> 401; garbage -> 401; foreign CA -> 401; expired -> 401;
//! valid permit with wrong subject -> 403; valid auditor permit -> 200 with
//! gateway entries, level filtering, source selection, and per-node fan-out.
//! A second gateway with `[logs].enabled=false` -> 404 even with a permit.
use std::sync::Arc;
use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD};
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
        Self { id: id.into(), sk, pk: pk.as_bytes().to_vec() }
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

async fn boot_gateway(
    dir: &std::path::Path,
    name: &str,
    ledger_addrs: Vec<String>,
    auth: AuthVerifier,
    logs_enabled: bool,
) -> (String, reqwest::Client) {
    let outbox = Outbox::open(dir.join(name).to_string_lossy().as_ref()).unwrap();
    let ledger = Arc::new(LedgerClient::new(ledger_addrs, 1500, 2000));
    let state = Arc::new(AppState {
        outbox,
        ledger,
        retry_batch: 16,
        auth,
        auditor_sub: "ledger-auditor".into(),
        logs_enabled,
    });
    let app = sagex_gateway::api::router(state);
    let port = free_port().await;
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await.unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    (format!("http://127.0.0.1:{port}"), reqwest::Client::new())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn auditor_logs_matrix() {
    let dir = tempfile::tempdir().unwrap();
    let ca = TestCa::generate("sagex-ca");
    let evil = TestCa::generate("evil-ca");

    // Seed the gateway-process ring buffer directly (no subscriber needed).
    sagex_ledger::LogBuffer::global().push_event("INFO", "logs-test", "marker-info-1");
    sagex_ledger::LogBuffer::global().push_event("ERROR", "logs-test", "marker-error-1");

    let ledger_ports = vec![free_port().await, free_port().await, free_port().await, free_port().await];
    let ledger_addrs: Vec<String> =
        ledger_ports.iter().map(|p| format!("127.0.0.1:{p}")).collect();

    let (gw_base, http) =
        boot_gateway(dir.path(), "gw-logs.db", ledger_addrs.clone(), ca.verifier(), true).await;
    let logs_url = format!("{gw_base}/logs");

    // 1. No token -> 401.
    let r = http.get(&logs_url).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // 2. Garbage token -> 401.
    let r = http.get(&logs_url).bearer_auth("not.a.permit").send().await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // 3. Foreign CA, right subject -> 401.
    let r = http
        .get(&logs_url)
        .bearer_auth(evil.permit("ledger-auditor", 3600))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // 4. Expired permit -> 401.
    let stale = ca.permit("ledger-auditor", 0);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let r = http.get(&logs_url).bearer_auth(stale).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // 5. Valid permit, wrong subject -> 403.
    let r = http
        .get(&logs_url)
        .bearer_auth(ca.permit("someone-else", 3600))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    // Boot the ledger for fan-out coverage.
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
            dir.path().join(format!("nlog{id}.db")).to_string_lossy().into_owned(),
            &identities[i].secret_b64(),
            &identities[i].public_b64,
            peers,
        );
        let node = sagex_ledger::node::Node::new(cfg).unwrap();
        handles.push(tokio::spawn(async move {
            let _ = node.run().await;
        }));
    }
    tokio::time::sleep(Duration::from_secs(3)).await;

    let auditor = ca.permit("ledger-auditor", 3600);

    // 6. Valid auditor permit -> 200, gateway markers present, 4 node tails.
    let v: serde_json::Value = http
        .get(&logs_url)
        .bearer_auth(&auditor)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let gw_msgs: Vec<String> = v["gateway"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(gw_msgs.iter().any(|m| m.contains("marker-info-1")), "{gw_msgs:?}");
    assert!(gw_msgs.iter().any(|m| m.contains("marker-error-1")), "{gw_msgs:?}");
    assert_eq!(v["nodes"].as_array().unwrap().len(), 4);
    for n in v["nodes"].as_array().unwrap() {
        assert!(n["node_addr"].is_string(), "{n:?}");
        assert!(n["entries"].is_array(), "{n:?}");
    }

    // 7. Level filter: only ERROR and above.
    let v: serde_json::Value = http
        .get(format!("{logs_url}?level=ERROR"))
        .bearer_auth(&auditor)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    for e in v["gateway"].as_array().unwrap() {
        assert_eq!(e["level"], "ERROR", "{e:?}");
    }
    assert!(v["gateway"].as_array().unwrap().iter().any(|e| e["message"]
        .as_str()
        .unwrap_or("")
        .contains("marker-error-1")));

    // 8. source=gateway: no node fan-out.
    let v: serde_json::Value = http
        .get(format!("{logs_url}?source=gateway"))
        .bearer_auth(&auditor)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(v["gateway"].is_array());
    assert_eq!(v["nodes"].as_array().unwrap().len(), 0);

    // 9. Disabled endpoint -> 404 even with a valid permit.
    let (off_base, _) =
        boot_gateway(dir.path(), "gw-logs-off.db", ledger_addrs, ca.verifier(), false).await;
    let r = http
        .get(format!("{off_base}/logs"))
        .bearer_auth(&auditor)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    for h in handles {
        h.abort();
    }
}
