# sagex-gateway — register gateway (sole writer)

HTTP REST front door for decryption records. Intake requires a
server-issued JOSE permit by default (CA-auth strategy, same code as
sagex-certauth via the `sagex-auth` crate): the permit signature, `kid`/`iss`
pin, expiry, `sub == record.user_id` binding, and single-use JTI are all
verified before anything is stored. Durable SQLite outbox with at-least-once
delivery to the PBFT ledger, plus forensic query proxying.
Fully offline/air-gapped.

## Layout

- `src/config.rs` — `gateway.toml` (server listen, ledger nodes, outbox, auth).
- `src/auth.rs` — permit verifier (CA-auth strategy via `sagex-auth`).
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

Register a decryption record (permit minted offline by sagex-certauth,
e.g. `sagex-certauth issue-permit --identity alice`):

```bash
curl -s -X POST 127.0.0.1:8081/register -H 'content-type: application/json' -d '{
  "permit":"<sagex-permit/v1 JWS>",
  "record":{"watermark":"wm-001","session_id":"sess-1","timestamp":1700000000000,
    "user_id":"alice","file_hash":"aa…","payload_hash":"bb…",
    "auth_server":"auth-1","signature":"base64-mldsa65"}}'
# 201 {"watermark":"wm-001","status":"committed","block_index":0,...}
# 202 {"status":"queued",...}       <- ledger unreachable, worker retries
# 400 invalid record  /  401 invalid permit (or permit required)
# 403 permit does not cover user_id  /  409 permit already used
# 502 ledger rejected
```

Forensic lookups (proxied to the ledger):

```bash
curl -s 127.0.0.1:8081/record/wm-001
curl -s '127.0.0.1:8081/records?user_id=alice'
curl -s 127.0.0.1:8081/block/0
curl -s 127.0.0.1:8081/status      # outbox backlog + per-node ledger status
curl -s 127.0.0.1:8081/outbox      # operator view of the queue
curl -s 127.0.0.1:8081/health
```

Restricted auditor logs (CA-issued credential, Bearer):

```bash
sagex-certauth issue-permit --identity ledger-auditor --ttl-hours 720
curl -s 127.0.0.1:8081/logs -H "Authorization: Bearer <permit>"
curl -s '127.0.0.1:8081/logs?level=WARN&limit=100' -H "Authorization: Bearer <permit>"
curl -s '127.0.0.1:8081/logs?source=gateway' -H "Authorization: Bearer <permit>"
```

`GET /logs` returns this process's ring buffer plus a per-node tail fanned
out over the ledger TCP `GetLogs` query (`?source=` selects `gateway`, one
node addr, or everything; `?level=` floors severity; `?limit=` caps at 500).
Access needs a permit whose subject equals `[logs].auditor_sub`
(default `ledger-auditor`) — same `sagex-auth` verification as intake, but
multi-use (no JTI burn); missing/invalid → 401, wrong subject → 403,
`[logs].enabled=false` → 404. The `sagex-ledger-ui` desktop app is the
intended viewer.

Fresh config template: `./target/debug/sagex-gateway init`.

## Semantics

- **Authenticated intake (default on)**: `POST /register` requires
  `{permit, record}`. The permit must be minted by the configured CA
  (`[auth].ca_id` + `ca_pubkey_b64`), be unexpired, have `sub ==
  record.user_id` (plain string equality — `user_id` stays free-form), and
  be unused (JTI burned in SQLite at intake, before the first submit, so a
  failed submit consumes its permit). Set `[auth].enabled=false` only for
  open demos; bare-record bodies are accepted only in that mode.
- **Permit jurisdiction**: intake accepts `kind=user` permits only (bound to
  `record.user_id`); auditor logs (`GET /logs`) accept `kind=server` permits
  only (bound to `auditor_sub`). A valid permit of the wrong kind is 403
  everywhere. Permits without a `kind` claim are rejected outright.
- **Gateway identity**: the RG ML-DSA-65 keypair is generated once into
  `gateway.db` and stable across restarts; its fingerprint is logged at boot
  and served on `GET /status`. The CA pins the public half (`RG_PK_B64`) to
  authenticate key-directory lookups — no syncing, no callbacks.
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
