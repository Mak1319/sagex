#!/usr/bin/env bash
# Start one ledger node instance: ./scripts/ledger-node-start.sh <N 1..4>
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
N="${1:?usage: ledger-node-start.sh <1|2|3|4>}"
PORT=$((8480 + N))
ENV_DIR="$ROOT/environments/ledger-node-$N"
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true
mkdir -p "$ENV_DIR"
[ -f "$ENV_DIR/.env" ] || { echo 'LEDGER_BEARER=change-me' > "$ENV_DIR/.env"; echo "wrote $ENV_DIR/.env — edit, re-run"; exit 1; }
[ -f "$ENV_DIR/config.toml" ] || sed -e "s/ledger-node-1/ledger-node-$N/" -e "s/127.0.0.1:8481/127.0.0.1:$PORT/" "$ROOT/crates2/sagex-ledger-node/sagex-ledger-node.toml.example" > "$ENV_DIR/config.toml"
[ -f "$ENV_DIR/ca-cert.pem" ] || { echo "missing $ENV_DIR/ca-cert.pem — provision CA cert, re-run"; exit 1; }
cargo build -p sagex-ledger-node
BINARY_PATH="$ROOT/target/debug/sagex-ledger-node" "$ROOT/run-isolate.sh" "$ENV_DIR" --config config.toml
