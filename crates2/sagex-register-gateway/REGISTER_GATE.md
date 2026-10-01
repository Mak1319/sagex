# REGISTER_GATE — what other agents need to register a watermark record

Target: `POST http://127.0.0.1:8490/register` (see `environments/register-gateway/config.toml`).

## 1. Preconditions (provisioned before service start)

- `environments/register-gateway/ca-cert.pem` exists (proper CA certificate).
- `environments/register-gateway/trusted-pubs/<recipient_id>.pub` exists for every
  allowed recipient. Filename MUST equal the keygen `--username` (e.g. `alice-test.pub`).
  File content MUST contain the recipient's ML-DSA-65 verifying-key hex.
- `.env` sets the bearer env var from config (`GATEWAY_BEARER`), proving CA-cert possession.

## 2. Required record fields

```json
{
  "recipient_id": "alice-test",
  "watermark_id": "hex, 32 bytes, unique per decrypt session",
  "doc_hash": "hex sha256 of source document",
  "recipient_dsa_pub": "hex ML-DSA-65 vk, must appear in trusted-pubs/<recipient_id>.pub",
  "session_id": "uuid/nonce of this decrypt event",
  "timestamp_ms": 1710000000000,
  "signature": "hex ML-DSA sig by recipient prv over recipient_id||watermark_id||doc_hash||session_id"
}
```

All 7 fields are mandatory. `recipient_id` must not contain `/` or `.`.

## 3. Register call

```
POST /register
Authorization: Bearer $GATEWAY_BEARER
Content-Type: application/json
<body = record above>
```

- `401` = missing/bad CA bearer. `403` = unknown `recipient_id` or pub mismatch.
- Accepted bearers: the `GATEWAY_BEARER` env token AND every non-comment line of
  `bearer_tokens_file` (`telemetry-bearers.txt` in the env dir) — desktop
  telemetry login keys go there, one per line.
- `400` = empty `watermark_id`/`signature`. `502` = no ledger node acked.
- Success: `{"acks": n, "block": {...height, block_hash...}}` (quorum = 3 of 4 nodes).

## 4. Telemetry / verification (open read, no cert)

- `GET /records` → full chain from first live node.
- `GET /records/:watermark_id` → block containing that watermark.
- `GET /verify/:watermark_id` → same block; verify `signature` with
  `recipient_dsa_pub` + check `block_hash` links into `/records` chain.

## 5. Node layout (4 nodes, one binary, per-instance envs)

```
environments/ledger-node-{1..4}/  config.toml .env <node>.prv <node>.pub chain.jsonl ca-cert.pem
ports: 8481 8482 8483 8484, quorum = 3
BINARY_PATH=target/debug/sagex-ledger-node ./run-isolate.sh environments/ledger-node-N --config config.toml
BINARY_PATH=target/debug/sagex-register-gateway ./run-isolate.sh environments/register-gateway --config config.toml
```

Node direct APIs: `GET /health`, `GET /chain`, `GET /record/:watermark_id`,
`POST /internal/append` (CA-gated, same bearer forwarded by gateway).
