use sagex_gateway::config::GatewayConfig;
use sagex_gateway::outbox::{Outbox, STATUS_DONE, STATUS_PENDING};
use sagex_ledger::model::DecryptionRecord;

fn sample_record(wm: &str) -> DecryptionRecord {
    DecryptionRecord {
        watermark: wm.into(),
        session_id: "sess-1".into(),
        timestamp: 1700000000000,
        user_id: "user-7".into(),
        file_hash: "aa".repeat(32),
        payload_hash: "bb".repeat(32),
        auth_server: "auth-1".into(),
        signature: "c2ln".into(),
    }
}

#[test]
fn config_template_parses_and_validates() {
    let tpl = GatewayConfig::template();
    let cfg: GatewayConfig = toml::from_str(&tpl).unwrap();
    assert!(cfg.validate().is_ok());
    assert_eq!(cfg.ledger.nodes.len(), 4);
}

#[test]
fn config_rejects_empty_nodes() {
    let mut cfg = GatewayConfig::default();
    cfg.ledger.nodes.clear();
    assert!(cfg.validate().is_err());
}

#[test]
fn outbox_upsert_is_idempotent() {
    let o = Outbox::open_in_memory().unwrap();
    let (e1, fresh1) = o.upsert_pending(&sample_record("wm-a")).unwrap();
    assert!(fresh1);
    assert_eq!(e1.status, STATUS_PENDING);
    let (e2, fresh2) = o.upsert_pending(&sample_record("wm-a")).unwrap();
    assert!(!fresh2);
    assert_eq!(e2.status, STATUS_PENDING);
}

#[test]
fn outbox_rejects_invalid_record() {
    let o = Outbox::open_in_memory().unwrap();
    let mut bad = sample_record("wm-bad");
    bad.signature.clear();
    assert!(o.upsert_pending(&bad).is_err());
    assert!(o.get("wm-bad").unwrap().is_none());
}

#[test]
fn outbox_done_short_circuits_resubmit() {
    let o = Outbox::open_in_memory().unwrap();
    o.upsert_pending(&sample_record("wm-done")).unwrap();
    o.set_inflight("wm-done").unwrap();
    o.mark_done("wm-done", 7, "deadbeef").unwrap();
    let e = o.get("wm-done").unwrap().unwrap();
    assert_eq!(e.status, STATUS_DONE);
    assert_eq!(e.block_index, Some(7));
    // Re-upsert returns the DONE entry (handler returns committed w/o resubmit).
    let (again, _) = o.upsert_pending(&sample_record("wm-done")).unwrap();
    assert_eq!(again.status, STATUS_DONE);
}

#[test]
fn outbox_claim_retry_cycle() {
    let o = Outbox::open_in_memory().unwrap();
    o.upsert_pending(&sample_record("wm-1")).unwrap();
    o.upsert_pending(&sample_record("wm-2")).unwrap();
    assert_eq!(o.pending_count().unwrap(), 2);
    let claimed = o.claim_pending(10).unwrap();
    assert_eq!(claimed.len(), 2);
    // Second claim finds nothing (rows are INFLIGHT now).
    assert!(o.claim_pending(10).unwrap().is_empty());
    // Transport failure -> back to PENDING with attempts bumped.
    o.mark_retryable("wm-1", "timeout").unwrap();
    o.mark_done("wm-2", 0, "h").unwrap();
    let e1 = o.get("wm-1").unwrap().unwrap();
    assert_eq!(e1.status, STATUS_PENDING);
    assert_eq!(e1.attempts, 1);
    assert_eq!(o.pending_count().unwrap(), 1);
    // Worker re-claims wm-1.
    let retry = o.claim_pending(10).unwrap();
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].watermark, "wm-1");
}

#[test]
fn outbox_failed_is_terminal_for_worker() {
    let o = Outbox::open_in_memory().unwrap();
    o.upsert_pending(&sample_record("wm-f")).unwrap();
    let c = o.claim_pending(10).unwrap();
    assert_eq!(c.len(), 1);
    o.mark_failed("wm-f", "ledger rejected").unwrap();
    assert!(o.claim_pending(10).unwrap().is_empty());
    assert_eq!(o.pending_count().unwrap(), 0);
    assert_eq!(o.list(10).unwrap().len(), 1);
}
