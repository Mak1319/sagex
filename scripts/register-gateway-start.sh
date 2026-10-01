#!/usr/bin/env bash
# Start the register gateway: ./scripts/register-gateway-start.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_DIR="$ROOT/environments/register-gateway"
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true
mkdir -p "$ENV_DIR/trusted-pubs"
[ -f "$ENV_DIR/.env" ] || { echo 'GATEWAY_BEARER=change-me' > "$ENV_DIR/.env"; echo "wrote $ENV_DIR/.env — edit, re-run"; exit 1; }
[ -f "$ENV_DIR/config.toml" ] || cp "$ROOT/crates2/sagex-register-gateway/sagex-register-gateway.toml.example" "$ENV_DIR/config.toml"
[ -f "$ENV_DIR/ca-cert.pem" ] || { echo "missing $ENV_DIR/ca-cert.pem — provision CA cert, re-run"; exit 1; }
cargo build -p sagex-register-gateway
BINARY_PATH="$ROOT/target/debug/sagex-register-gateway" "$ROOT/run-isolate.sh" "$ENV_DIR" --config config.toml
