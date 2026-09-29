# Register gate — ingress contract (sole writer)

The register gate is **not** implemented here. It is the only component allowed to
submit decryption records. Any node accepts `Submit` over TCP and forwards to the
PBFT leader; all nodes commit the same block (`1 record = 1 block`).

## Transport

- TCP, newline-delimited JSON (one object per line), UTF-8.
- No auth on the socket (air-gapped network assumed). If needed, run behind the
  gate's own mTLS / firewall — out of scope for the ledger.
- `req_id` (integer, optional) is echoed back in the response for correlation.
- `from` (integer or null) identifies peer nodes; gates send `null`.

## Submit

```json
{"req_id":1,"from":null,"type":"Submit","record":{
  "watermark":"wm-9f3a…",
  "session_id":"sess-2026-…",
  "timestamp":1727600000000,
  "user_id":"alice",
  "file_hash":"<hex sha256 of source file>",
  "payload_hash":"<hex sha256 of watermarked payload>",
  "auth_server":"auth-1",
  "signature":"<base64 ML-DSA-65 by recipient over canonical record>"
}}
```

Aliases accepted: `sessionid`, `userid`, `file_hash`/`fileHash`,
`payload_hash`/`payloadHash`, `auth_server`/`authServer`.

Canonical bytes bound by `signature` and by the block hash:

```json
{"watermark":"…","session_id":"…","timestamp":…,"user_id":"…",
 "file_hash":"…","payload_hash":"…","auth_server":"…"}
```

(field order fixed as above, compact `serde_json` encoding).

### Reply

```json
{"req_id":1,"type":"Reply","ok":true,"block_index":7,
 "block_hash":"…","error":null}
```

- `ok:false` + `error` on validation failure (`watermark` empty, bad hashes,
  missing `signature`, …).
- Duplicate `watermark` returns the existing `block_index`/`block_hash` with
  `error:"duplicate: already committed"` (idempotent retry safe).
- After a view change the gate should **resubmit** unacknowledged records.

## Forensic queries (TCP only)

```json
{"req_id":2,"from":null,"type":"QueryWatermark","watermark":"wm-9f3a…"}
{"req_id":3,"from":null,"type":"QueryUser","user_id":"alice"}
{"req_id":4,"from":null,"type":"GetBlock","index":7}
{"req_id":5,"from":null,"type":"Status"}
```

Responses:

```json
{"req_id":2,"type":"QueryResult","blocks":[{ "header":{ "index":7,
  "prev_hash":"…","hash":"…","timestamp":…,"view":0,"seq":8,"proposer":0 },
  "record":{…} }],"error":null}
{"req_id":5,"type":"StatusResult","node_id":0,"height":8,
 "tip_hash":"…","view":0,"seq":9}
```

`blocks:[]` + `error:"not found"` when nothing matches.

## Gate responsibilities

1. Build the `DecryptionRecord` at decryption time (unique `watermark` per session).
2. Have the recipient sign the canonical record with their ML-DSA-65 private key.
3. `Submit` to any node; wait for `Reply{ok:true}` (retry on timeout/duplicate).
4. For leak attribution: extract watermark → `QueryWatermark` → verify block hash
   chain (`verify --db …`) + recipient `signature` against the ledger record.
