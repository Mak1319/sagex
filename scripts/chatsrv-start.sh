#!/usr/bin/env bash
# Start the sagex-chatsrv compiled-service.
# - builds sagex-keygen + sagex-chatsrv (one crate at a time, low RAM)
# - bootstraps environments/chatsrv/{config.toml,.env,keys} if missing
#   (keys via the keygen CLI, NOT inside the service)
# - starts the binary isolated via run-isolate.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_DIR="$ROOT/environments/chatsrv"
SERVICE_USER="chatsrv-service"

export CARGO_BUILD_JOBS=1
export CARGO_NET_OFFLINE=true

mkdir -p "$ENV_DIR"

# 1. .env must provide KEY_PASSWORD (+ JWT_SECRET).
if [ ! -f "$ENV_DIR/.env" ]; then
  cat > "$ENV_DIR/.env" <<'EOF'
# Secrets for the chatsrv test env. NEVER commit (gitignored).
KEY_PASSWORD=change-me-before-use
JWT_SECRET=change-me-before-use-min-32-chars
MONGO_ROOT_USER=sagex
MONGO_ROOT_PASS=sagex-dev-only
RUSTFS_ACCESS_KEY=sagexadmin
RUSTFS_SECRET_KEY=sagex-dev-only-change-me
EOF
  echo "wrote $ENV_DIR/.env — edit KEY_PASSWORD/JWT_SECRET, then re-run"
  exit 1
fi
# shellcheck disable=SC1091
set -a; . "$ENV_DIR/.env"; set +a

# 2. config.toml ships from the tracked template if missing.
if [ ! -f "$ENV_DIR/config.toml" ]; then
  cp "$ROOT/crates2/sagex-chatsrv/config.toml.example" "$ENV_DIR/config.toml"
  echo "wrote $ENV_DIR/config.toml from template"
fi

# 3. Service keypair: generate via keygen CLI when absent.
if [ ! -f "$ENV_DIR/$SERVICE_USER.prv" ] || [ ! -f "$ENV_DIR/$SERVICE_USER.pub" ]; then
  cargo build -p sagex-keygen
  KEY_PASSWORD="$KEY_PASSWORD" \
    "$ROOT/target/debug/sagex-keygen" \
      --username "$SERVICE_USER" \
      --out-dir "$ENV_DIR" \
      --no-tpm \
      --password-env KEY_PASSWORD \
      --force
  # Point the config at the generated files (relative to env dir).
  sed -i "s|^prv_path = .*|prv_path = \"$SERVICE_USER.prv\"|" "$ENV_DIR/config.toml"
  sed -i "s|^pub_path = .*|pub_path = \"$SERVICE_USER.pub\"|" "$ENV_DIR/config.toml"
fi

# 4. Build the service, then run isolated (env dir = HOME).
cargo build -p sagex-chatsrv
BINARY_PATH="$ROOT/target/debug/sagex-chatsrv" \
  "$ROOT/run-isolate.sh" "$ENV_DIR" --config "$ENV_DIR/config.toml" "$@"
