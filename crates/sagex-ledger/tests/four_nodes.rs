//! 4-node PBFT integration: submit via follower (forwarded to leader),
//! expect commit on all nodes + watermark lookup. Then kill 1 node,
//! submit again, expect remaining 3 still reach quorum (f=1).
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use sagex_ledger::config::{ConsensusSection, NodeConfig, NodeSection, PeerEntry};
use sagex_ledger::identity::Identity;
use sagex_ledger::model::DecryptionRecord;
use sagex_ledger::node::Node;
use sagex_ledger::proto::{Envelope, Message, Response, ResponseEnvelope};

async fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port()
}

fn record(wm: &str, user: &str) -> DecryptionRecord {
    DecryptionRecord {
        watermark: wm.into(),
        session_id: "sess-int".into(),
        timestamp: 1700000000000,
        user_id: user.into(),
        file_hash: "aa".repeat(32),
        payload_hash: "bb".repeat(32),
        auth_server: "auth-test".into(),
        signature: "c2lnbmF0dXJl".into(),
    }
}

async fn send_envelope(addr: &str, env: Envelope) -> ResponseEnvelope {
    let sock = TcpStream::connect(addr).await.unwrap();
    let (rh, mut wh) = sock.into_split();
    let mut line = serde_json::to_string(&env).unwrap();
    line.push('\n');
    wh.write_all(line.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(rh).lines();
    let resp_line = tokio::time::timeout(Duration::from_secs(15), lines.next_line())
        .await
        .expect("reply timeout")
        .unwrap()
        .expect("eof");
    serde_json::from_str(&resp_line).unwrap()
}

async fn wait_for_watermark(addr: &str, wm: &str) {
    for _ in 0..50 {
        let env = Envelope {
            req_id: Some(99),
            from: None,
            msg: Message::QueryWatermark { watermark: wm.into() },
        };
        if let Ok(resp) = tokio::time::timeout(Duration::from_secs(2), send_envelope(addr, env)).await {
            if let Ok(r) = tokio::time::timeout(Duration::from_secs(1), async { resp }).await {
                if matches!(&r.resp, Response::QueryResult { blocks, .. } if !blocks.is_empty()) {
                    return;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("watermark {wm} never appeared on {addr}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn four_nodes_commit_and_survive_one_fault() {
    // --- keys + addrs ---
    let ports: Vec<u16> = vec![free_port().await, free_port().await, free_port().await, free_port().await];
    let addrs: Vec<String> = ports.iter().map(|p| format!("127.0.0.1:{p}")).collect();
    let ids: Vec<u64> = vec![0, 1, 2, 3];
    let identities: Vec<Identity> = ids.iter().map(|i| Identity::generate(*i)).collect();

    let dir = tempfile::tempdir().unwrap();
    let mut handles = vec![];
    for (i, id) in ids.iter().enumerate() {
        let mut peers = vec![];
        for (j, pid) in ids.iter().enumerate() {
            if i == j {
                continue;
            }
            peers.push(PeerEntry {
                id: *pid,
                addr: addrs[j].clone(),
                pubkey: identities[j].public_b64.clone(),
            });
        }
        let cfg = NodeConfig {
            node: NodeSection {
                id: *id,
                listen: addrs[i].clone(),
                db: dir.path().join(format!("n{id}.db")).to_string_lossy().into_owned(),
                secret_key: identities[i].secret_b64(),
                pubkey: identities[i].public_b64.clone(),
            },
            peers,
            consensus: ConsensusSection { f: 1, view_timeout_ms: 8000 },
        };
        let node = Node::new(cfg).unwrap();
        handles.push(tokio::spawn(async move {
            let _ = node.run().await;
        }));
    }
    // let mesh form
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Submit via node 1 (replica when view=0 leader=0) to exercise forwarding.
    let env = Envelope {
        req_id: Some(1),
        from: None,
        msg: Message::Submit { record: record("wm-int-1", "alice") },
    };
    let resp = send_envelope(&addrs[1], env).await;
    match resp.resp {
        Response::Reply { ok, block_hash, error, .. } => {
            assert!(ok, "submit failed: {error:?}");
            assert!(block_hash.is_some());
        }
        other => panic!("unexpected: {other:?}"),
    }

    // All 4 nodes should have it.
    for a in &addrs {
        wait_for_watermark(a, "wm-int-1").await;
    }

    // Kill node 3 (faulty/offline). Remaining 3 must still reach quorum=3.
    handles[3].abort();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let env2 = Envelope {
        req_id: Some(2),
        from: None,
        msg: Message::Submit { record: record("wm-int-2", "bob") },
    };
    let resp2 = send_envelope(&addrs[0], env2).await;
    match resp2.resp {
        Response::Reply { ok, error, .. } => assert!(ok, "post-fault submit failed: {error:?}"),
        other => panic!("unexpected: {other:?}"),
    }
    for a in &addrs[..3] {
        wait_for_watermark(a, "wm-int-2").await;
    }

    // Forensic query by user on survivor.
    let q = Envelope {
        req_id: Some(3),
        from: None,
        msg: Message::QueryUser { user_id: "alice".into() },
    };
    let qr = send_envelope(&addrs[0], q).await;
    match qr.resp {
        Response::QueryResult { blocks, error } => {
            assert!(error.is_none(), "{error:?}");
            assert_eq!(blocks.len(), 1);
            assert_eq!(blocks[0].record.watermark, "wm-int-1");
        }
        other => panic!("unexpected: {other:?}"),
    }

    // HashMap import sanity (keeps peer map types referenced in test scope)
    let _m: HashMap<u64, String> = HashMap::new();
    for h in handles {
        h.abort();
    }
}
