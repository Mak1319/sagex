use sagex_ledger::model::{Block, DecryptionRecord};

fn sample_record(wm: &str) -> DecryptionRecord {
    DecryptionRecord {
        watermark: wm.into(),
        session_id: "sess-1".into(),
        timestamp: 1700000000000,
        user_id: "user-42".into(),
        file_hash: "aa".repeat(32),
        payload_hash: "bb".repeat(32),
        auth_server: "auth-1".into(),
        signature: "c2ln".into(),
    }
}

#[test]
fn record_validation_rejects_empty_watermark() {
    let mut r = sample_record("");
    assert!(r.validate().is_err());
    r = sample_record("wm-1");
    assert!(r.validate().is_ok());
}

#[test]
fn block_hash_chain_verifies() {
    let r1 = sample_record("wm-1");
    let b1 = Block::genesis(r1, 0);
    assert!(b1.verify_link("GENESIS"));
    let r2 = sample_record("wm-2");
    let b2 = Block::next(&b1.header, 0, 1, 0, r2);
    assert!(b2.verify_link(&b1.header.hash));
    assert!(!b2.verify_link("wrong"));
}

#[test]
fn store_insert_and_lookup() {
    let s = sagex_ledger::Store::open_in_memory().unwrap();
    let b = Block::genesis(sample_record("wm-lookup"), 0);
    s.insert_block(&b).unwrap();
    let got = s.get_by_watermark("wm-lookup").unwrap().unwrap();
    assert_eq!(got.header.hash, b.header.hash);
    assert_eq!(s.verify_chain().unwrap(), 1);
    // duplicate watermark rejected
    let b2 = Block::next(&b.header, 0, 1, 0, sample_record("wm-lookup"));
    assert!(s.insert_block(&b2).is_err());
}

#[test]
fn pbft_happy_path_single_node_quorum1() {
    use sagex_ledger::identity::Identity;
    use sagex_ledger::pbft::PbftAction;
    // f=0, quorum=1: leader proposes and commits immediately.
    let id = Identity::generate(0);
    let mut pbft = sagex_ledger::pbft::Pbft::new(0, 0, 1, 1, Box::new(|_| 0));
    let rec = sample_record("wm-pbft");
    let acts = pbft.propose(rec, None, &id).unwrap();
    // Expect PrePrepare broadcast + Prepare broadcast + Commit action.
    let mut saw_commit = false;
    for a in acts {
        if let PbftAction::Commit(b) = a {
            assert_eq!(b.record.watermark, "wm-pbft");
            saw_commit = true;
        }
    }
    assert!(saw_commit, "leader should commit with quorum=1");
}

#[test]
fn config_quorum_validation() {
    let toml_str = r#"
[node]
id = 0
listen = "127.0.0.1:7000"
db = ":memory:"
secret_key = "x"
pubkey = "y"
[[peers]]
id = 1
addr = "127.0.0.1:7001"
[[peers]]
id = 2
addr = "127.0.0.1:7002"
[[peers]]
id = 3
addr = "127.0.0.1:7003"
[consensus]
f = 1
view_timeout_ms = 8000
"#;
    let cfg: sagex_ledger::NodeConfig = toml::from_str(toml_str).unwrap();
    assert!(cfg.validate().is_ok());
    assert_eq!(cfg.quorum(), 3);
    assert_eq!(cfg.leader_of(0), 0);
    assert_eq!(cfg.leader_of(1), 1);
}
