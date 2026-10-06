# sagex-nodeblock

An offline ledger-node service for gateway-submitted decryption metadata. The service does not
store documents and does not validate recipient certificates or recipient signatures. It stores
those values exactly as supplied by the trusted gateway.

## Build and start

Build the compiled service from the repository root:

```sh
CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true cargo build -p sagex-nodeblock
```

Create `environments/sagex-nodeblock/`, copy `sagex-nodeblock.toml.example` there as `config.toml`,
and replace all certificate and key placeholders. Build, then start the compiled binary in its
isolated environment:

```sh
CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true cargo build -p sagex-nodeblock
crates/sagex-nodeblock/run-isolate.sh environments/sagex-nodeblock --config config.toml
```

The environment and its SQLite database/WAL files persist across restarts. The isolation runner
shares the configured air-gapped network so the gateway and validators can connect; network
firewall rules must keep the API listener gateway-only and the peer listener validator-only.

Derive the public key to place in the other validators' configurations with
`sagex-nodeblock --public-key-hex <64-character-seed-hex>`. Create and distribute each seed
offline; never put real seeds in the repository.

The API listener requires a client certificate issued by the configured gateway CA. The peer
listener requires a client certificate issued by the validator CA. Peer endpoints are configured
statically. Firewall rules should allow the API listener only from the gateway and the peer
listener only from configured validator hosts. TLS is restricted to TLS 1.3 with ML-KEM-768 key
exchange; the gateway and validator clients must support that group.

## Gateway API

- `POST /v1/records`: commit a metadata record.
- `GET /v1/records/{watermark}`: return the committed record and its validator commit certificate.
- `GET /v1/health`: return node height and record count.

The record fields are `watermark`, `file_hash`, `username`, `certificate`, `filename`, `session_id`,
`recipient_signature`, `file_signature`, and `event_time`. Only field presence and JSON shape are
checked. Recipient cryptography is intentionally trusted to the gateway.

## Consensus limitations

The current peer protocol uses one durable vote lock per height and a two-thirds-plus-one ML-DSA-65
quorum certificate. Nodes catch up missing committed blocks from peers. The protocol does not yet
implement view changes; a malicious validator can equivocate and leave honest validators with
conflicting vote locks, stopping progress at that height. Treat this as an initial ledger-node
foundation, not a complete Byzantine-fault-tolerant production consensus system.
