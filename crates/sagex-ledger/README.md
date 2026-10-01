# sagex-ledger — PBFT blockchain nodes server

Permissioned BFT ledger for decryption-audit records. Fully offline/air-gapped:
TCP JSON-lines transport, SQLite storage, Tokio async, ML-DSA-65 node identities,
1 record = 1 block, PBFT (pre-prepare / prepare / commit, quorum `2f+1`).

## Layout

- `src/model.rs` — `DecryptionRecord` payload (`watermark, sessionid, timestamp,
  userid, file hash, payload hash, auth server, signature`) + `Block` hash chain.
- `src/store.rs` — SQLite per node (`blocks`, `records`), `verify_chain()`.
- `src/proto.rs` — JSON-lines `Message` / `Response` / envelopes.
- `src/pbft.rs` — pure state machine (IO-free, unit-tested).
- `src/net.rs` — Tokio TCP listener, peer dialers, Hello handshake, client handling.
- `src/node.rs` — event loop gluing consensus + store + network.
- `src/identity.rs` — ML-DSA-65 node keys (base64 in TOML, no KMS).
- `src/config.rs` — `NodeConfig` (arbitrary ids/addrs, `N >= 3f+1` checked).
- `config/nodes/node{0..3}.toml` — 4-node DEV cluster (example keys only).
- `REGISTER_GATE.md` — wire contract for the register gate (sole writer).
- `tests/unit.rs`, `tests/four_nodes.rs` — unit + 4-node fault-tolerance test.

## Quick start (4 nodes, f=1)

```bash
cargo build -p sagex-ledger
./target/debug/sagex-ledger-node run --config crates/sagex-ledger/config/nodes/node0.toml &
./target/debug/sagex-ledger-node run --config crates/sagex-ledger/config/nodes/node1.toml &
./target/debug/sagex-ledger-node run --config crates/sagex-ledger/config/nodes/node2.toml &
./target/debug/sagex-ledger-node run --config crates/sagex-ledger/config/nodes/node3.toml &
```

Submit (register gate) — any node, forwarded to the leader:

```bash
echo '{"req_id":1,"from":null,"type":"Submit","record":{"watermark":"wm-001","session_id":"sess-1","timestamp":1700000000000,"user_id":"alice","file_hash":"aa…","payload_hash":"bb…","auth_server":"auth-1","signature":"base64-mldsa65"}}' | nc 127.0.0.1 7001
```

Forensic lookup (TCP only):

```bash
echo '{"req_id":2,"from":null,"type":"QueryWatermark","watermark":"wm-001"}' | nc 127.0.0.1 7000
echo '{"req_id":3,"from":null,"type":"QueryUser","user_id":"alice"}' | nc 127.0.0.1 7000
echo '{"req_id":4,"from":null,"type":"Status"}' | nc 127.0.0.1 7000
```

Verify a node's hash chain offline:

```bash
./target/debug/sagex-ledger-node verify --db data/node0.db
```

## Fresh keys / custom cluster

```bash
./target/debug/sagex-ledger-node init --id 5 --listen 10.0.0.5:7000 --db data/node5.db
# copy each node's pubkey into every other node's [[peers]] section
# ensure N >= 3f+1 (e.g. 4 nodes for f=1, 7 nodes for f=2)
```

## Tests

```bash
cargo test -p sagex-ledger
```

`four_nodes` spins 4 ephemeral nodes, submits via a follower, asserts all commit +
lookup, kills 1 node, asserts the remaining 3 still reach quorum.
