# sagex-gateway — register gateway (sole writer)

HTTP REST front door for decryption records. Pass-through design: structural
validation only (no signature verification, no key lookup), durable SQLite
outbox with at-least-once delivery to the PBFT ledger, plus forensic query
proxying. Fully offline/air-gapped.

## Layout

- `src/config.rs` — `gateway.toml` (server listen, ledger nodes, outbox).
- `src/outbox.rs` — SQLite queue (`PENDING|INFLIGHT|DONE|FAILED`), idempotent on watermark.
- `src/ledger_client.rs` — TCP JSON-lines client reusing `sagex-ledger` protocol; rotating failover across nodes.
- `src/api.rs` — axum routes + background retry worker.
- `config/gateway.toml` — example config. `tests/unit.rs`, `tests/integration.rs`.

## Quick start (with the 4-node ledger)

```bash
cargo build -p sagex-ledger -p sagex-gateway
# 4 ledger nodes (see ../sagex-ledger/README.md)
for i in 0 1 2 3; do
  ./target/debug/sagex-ledger-node run --config crates/sagex-ledger/config/nodes/node$i.toml &
done
# gateway
./target/debug/sagex-gateway run --config crates/sagex-gateway/config/gateway.toml &
```

Register a decryption record:

```bash
curl -s -X POST 127.0.0.1:8080/register -H 'content-type: application/json' -d '{
  "watermark":"wm-001","session_id":"sess-1","timestamp":1700000000000,
  "user_id":"alice","file_hash":"aa…","payload_hash":"bb…",
  "auth_server":"auth-1","signature":"base64-mldsa65"}'
# 201 {"watermark":"wm-001","status":"committed","block_index":0,...}
# 202 {"status":"queued",...}       <- ledger unreachable, worker retries
# 400 invalid record  /  502 ledger rejected
```

Forensic lookups (proxied to the ledger):

```bash
curl -s 127.0.0.1:8080/record/wm-001
curl -s '127.0.0.1:8080/records?user_id=alice'
curl -s 127.0.0.1:8080/block/0
curl -s 127.0.0.1:8080/status      # outbox backlog + per-node ledger status
curl -s 127.0.0.1:8080/outbox      # operator view of the queue
curl -s 127.0.0.1:8080/health
```

Fresh config template: `./target/debug/sagex-gateway init`.

## Semantics

- **Idempotent**: resubmitting a watermark returns the stored commit (`duplicate:true`), never a second block — matches ledger dedup.
- **Durable**: the record hits SQLite before the first submit attempt; gate restarts and ledger outages just delay `DONE`. The worker sweeps `PENDING` rows every `retry_interval_ms`.
- **Failover**: each submit/query rotates across `ledger.nodes`; any single dead node (f=1) is invisible to callers.
- **Terminal failures**: a ledger `Reply{ok:false}` (shouldn't happen after local validation) marks the row `FAILED` for operator inspection via `/outbox`, not retried.

## Tests

```bash
cargo test -p sagex-gateway
```

`integration` boots the gateway with the ledger down (asserts `202 queued` +
outbox row), then boots 4 nodes (asserts auto-drain, duplicate handling, live
submit, user query, 1-fault survival, and `400` on bad input).
