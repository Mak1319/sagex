#!/usr/bin/env bash
# Start the full sagex backend stack (no desktop UI).
#
# Boot order (forced by dependencies):
#   1. mongodb (docker compose, both Mongo DBs ride it: chatdbx, sagexcadb)
#   2. 4x sagex-ledger-node (7000-7003, PBFT mesh)
#   3a. gateway pre-boot, auth disabled (generates stable RG keypair in gateway.db)
#   3b. sagex-certauth (127.0.0.1:8082; pins BLS + RG public keys, strict mode)
#   4. sagex-gateway (127.0.0.1:8081; config auto-wired with the CA public key)
#   5. sagex-chatsrv (0.0.0.0:8080 with pinned ML-DSA keys; the port sagex_ui points at)
#
# Usage:
#   ./scripts/start.sh                  # debug binaries (see scripts/build.sh)
#   PROFILE=release ./scripts/start.sh  # release binaries
#
# Env (all optional except via .env — explicit env always wins):
#   PROFILE        debug|release (default: debug)
#   CERTAUTH_PORT  default: 8082 (chatsrv keeps 8080, gateway 8081)
#   CHAT_BIND      default: 0.0.0.0:8080
#   GATEWAY_ADDR   default: 127.0.0.1:8081
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROFILE="${PROFILE:-debug}"
CERTAUTH_PORT="${CERTAUTH_PORT:-8082}"
CHAT_BIND="${CHAT_BIND:-0.0.0.0:8080}"
GATEWAY_ADDR="${GATEWAY_ADDR:-127.0.0.1:8081}"
BINDIR="target/${PROFILE}"
LOGDIR="logs"
PIDDIR=".pids"
RUNDIR="runtime"
MONGO_CONTAINER="sagexca-mongodb"

mkdir -p "$LOGDIR" "$PIDDIR" "$RUNDIR"

log()  { echo "==> $*"; }
fail() { echo "error: $*" >&2; exit 1; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}
need_cmd docker
need_cmd python3
need_cmd curl
docker compose version >/dev/null 2>&1 || fail "docker compose plugin not found"

# --- root .env (single source of truth for secrets) ---
if [[ ! -f .env ]]; then
  [[ -f .env.example ]] || fail ".env.example missing; cannot bootstrap config"
  cp .env.example .env
  echo "WARN: no .env found — copied from .env.example." >&2
  echo "WARN: change the *_PASSWORD values before any real deployment." >&2
fi
set -a
# shellcheck disable=SC1091
source .env
set +a

MONGO_PORT="${MONGO_PORT:-27017}"
CA_KEY_DIR="${CA_KEY_DIR:-./ca-data}"
CA_IDENTITY="${CA_IDENTITY:-sagex-ca}"
[[ -n "${CA_PASSWORD:-}" ]] || fail "CA_PASSWORD is not set (see .env)"
[[ -n "${MONGO_ROOT_USER:-}" && -n "${MONGO_ROOT_PASSWORD:-}" ]] \
  || fail "MONGO_ROOT_USER/MONGO_ROOT_PASSWORD are not set (see .env)"

# --- port guard: script-managed ports must be free (mongo is compose-owned:
# `docker compose up` below is idempotent, so leave 27017 out of the guard) ---
port_open() { timeout 1 bash -c "echo > /dev/tcp/127.0.0.1/$1" 2>/dev/null; }
for p in 7000 7001 7002 7003 8080 8081 "$CERTAUTH_PORT"; do
  if port_open "$p"; then
    fail "port $p is already in use — stop whatever holds it first (see scripts/stop.sh)"
  fi
done

# --- binaries (built by scripts/build.sh) ---
for bin in sagex-ledger-node sagex-gateway sagex-certauth sagex-chatsrv; do
  [[ -x "${BINDIR}/${bin}" ]] \
    || fail "missing ${BINDIR}/${bin} — run ./scripts/build.sh first"
done

# --- 0. chatsrv (BLS) keypair: pinned across runs, CA pins the public half ---
# Generated once via sagex-ledger-node init (same ML-DSA-65 keys), stored in
# runtime/ (gitignored, chmod 600). Without this chatsrv would mint ephemeral
# keys and break every CA pin on every restart.
CHAT_KEYS="${RUNDIR}/chatsrv-ml-dsa.env"
if [[ ! -f "$CHAT_KEYS" ]]; then
  log "generating pinned chatsrv ML-DSA-65 keypair"
  INIT_OUT="$("${BINDIR}/sagex-ledger-node" init --id 99 --listen 127.0.0.1:7999 --db /tmp/sagex-chat-keys.db)" \
    || fail "chatsrv keypair generation failed"
  # -m1: the init template also shows a peer pubkey placeholder — take only
  # the node's own keys (first match each). `|| true` so a format change
  # surfaces as the explicit emptiness error below, not a silent exit.
  CHAT_SK="$(echo "$INIT_OUT" | grep -m1 '^secret_key = ' | cut -d'"' -f2 || true)"
  CHAT_PK="$(echo "$INIT_OUT" | grep -m1 '^pubkey = ' | cut -d'"' -f2 || true)"
  rm -f /tmp/sagex-chat-keys.db
  [[ -n "$CHAT_SK" && -n "$CHAT_PK" ]] || fail "could not generate chatsrv keypair"
  {
    echo "MLDSA65_SK_B64=${CHAT_SK}"
    echo "MLDSA65_PK_B64=${CHAT_PK}"
  } >"$CHAT_KEYS"
  chmod 600 "$CHAT_KEYS"
fi
# shellcheck disable=SC1091
source "$CHAT_KEYS"
[[ -n "${MLDSA65_SK_B64:-}" && -n "${MLDSA65_PK_B64:-}" ]] \
  || fail "bad $CHAT_KEYS (delete it to regenerate)"
# NOTE: SK/PK stay shell-local on purpose (never exported); they are passed
# inline only to the chatsrv process below.
export MLDSA65_KID="${MLDSA65_KID:-sagex-chatsrv-mldsa65-01}"

wait_for_port() { # host port timeout_secs
  local host="$1" port="$2" timeout="$3" i=0
  while ! timeout 1 bash -c "echo > /dev/tcp/${host}/${port}" 2>/dev/null; do
    i=$((i + 1))
    [[ "$i" -ge "$timeout" ]] && return 1
    sleep 1
  done
  return 0
}

wait_for_http() { # url timeout_secs
  local url="$1" timeout="$2" i=0
  while ! curl -sf -m 3 "$url" >/dev/null 2>&1; do
    i=$((i + 1))
    [[ "$i" -ge "$timeout" ]] && return 1
    sleep 1
  done
  return 0
}

# --- 1. mongodb ---
log "starting mongodb (docker compose)"
docker compose up -d mongodb >/dev/null
log "waiting for mongodb to be healthy"
i=0
while [[ "$(docker inspect --format '{{.State.Health.Status}}' "$MONGO_CONTAINER" 2>/dev/null)" != "healthy" ]]; do
  i=$((i + 1))
  [[ "$i" -ge 90 ]] && fail "mongodb never became healthy (see: docker logs $MONGO_CONTAINER)"
  sleep 1
done
log "mongodb healthy"

# --- 1b. minio (object storage for chat media) ---
# Best-effort: the image may be unpullable offline (like this machine).
# If minio boots, chatsrv uses it; otherwise chatsrv runs with an in-memory
# fake (text chat unaffected) and media URLs won't resolve. Either way LOUD.
MINIO_UP=false
MINIO_PORT="${MINIO_PORT:-9000}"
MINIO_CONSOLE_PORT="${MINIO_CONSOLE_PORT:-9001}"
if port_open "$MINIO_PORT" || port_open "$MINIO_CONSOLE_PORT"; then
  echo "WARN: ports 9000/9001 busy — skipping minio, chatsrv media will use in-memory fake" >&2
elif docker compose up -d minio >/dev/null 2>&1; then
  log "waiting for minio to be healthy"
  i=0
  while [[ "$(docker inspect --format '{{.State.Health.Status}}' sagex-minio 2>/dev/null)" != "healthy" ]]; do
    i=$((i + 1))
    if [[ "$i" -ge 60 ]]; then
      echo "WARN: minio image present but never healthy — continuing with in-memory fake (see: docker logs sagex-minio)" >&2
      break
    fi
    sleep 1
  done
  if [[ "$(docker inspect --format '{{.State.Health.Status}}' sagex-minio 2>/dev/null)" == "healthy" ]]; then
    MINIO_UP=true
    log "minio healthy"
  fi
else
  echo "WARN: could not start minio (image unpullable offline?) — chatsrv media will use in-memory fake" >&2
fi

# --- 2. ledger nodes ---
log "starting 4 ledger nodes (7000-7003)"
for n in 0 1 2 3; do
  cfg="crates/sagex-ledger/config/nodes/node${n}.toml"
  [[ -f "$cfg" ]] || fail "missing ledger config: $cfg (copy from ${cfg}.example and fill keys via 'sagex-ledger-node init' — real configs are local-only, see .gitignore)"
  nohup "${BINDIR}/sagex-ledger-node" run --config "$cfg" \
    >"${LOGDIR}/ledger-node${n}.log" 2>&1 &
  echo "$!" >"${PIDDIR}/ledger-node${n}.pid"
done
for p in 7000 7001 7002 7003; do
  wait_for_port 127.0.0.1 "$p" 30 || fail "ledger node on $p never opened (see logs/ledger-node*.log)"
done
sleep 3 # let the PBFT mesh form
log "ledger mesh up"

# --- 3a. gateway pre-boot, auth disabled (stable RG keypair generation) ---
# The CA needs RG_PK_B64 at ITS boot, but the RG key is born at gateway boot.
# So: boot once without auth, scrape the logged public key, shut down. The
# final boot below reuses the same gateway.db, hence the same RG key.
GW_TEMPLATE="crates/sagex-gateway/config/gateway.toml"
[[ -f "$GW_TEMPLATE" ]] || fail "missing gateway template: $GW_TEMPLATE"
python3 - "$GW_TEMPLATE" "${RUNDIR}/gateway-preboot.toml" "$GATEWAY_ADDR" <<'EOF'
import sys
_, template, output, listen = sys.argv
section = None
out = []
for line in open(template):
    s = line.strip()
    if s.startswith("[") and s.endswith("]"):
        section = s[1:-1]
    if section == "server" and line.startswith("listen = "):
        line = f'listen = "{listen}"\n'
    if section == "auth" and line.startswith("enabled = "):
        line = "enabled = false\n"
    out.append(line)
open(output, "w").writelines(out)
EOF
log "pre-booting gateway (auth disabled) to obtain RG public key"
nohup "${BINDIR}/sagex-gateway" run --config "${RUNDIR}/gateway-preboot.toml" \
  >"${LOGDIR}/gateway-preboot.log" 2>&1 &
echo "$!" >"${PIDDIR}/gateway-preboot.pid"
wait_for_http "http://${GATEWAY_ADDR}/health" 60 \
  || fail "pre-boot gateway never healthy (see logs/gateway-preboot.log)"
# Read the RG public key straight from gateway.db (robust: no log scraping).
# Same DB file serves the final boot below, hence the same RG key.
RG_PK_B64=""
for _ in $(seq 1 30); do
  RG_PK_B64="$(python3 -c "
import sqlite3, sys
try:
    c = sqlite3.connect('data/gateway.db')
    row = c.execute(\"SELECT public_b64 FROM rg_identity WHERE id='rg'\").fetchone()
    if row and row[0]:
        print(row[0])
except Exception:
    pass
" 2>/dev/null || true)"
  [[ -n "$RG_PK_B64" ]] && break
  sleep 1
done
[[ -n "$RG_PK_B64" ]] || fail "RG public key never appeared in data/gateway.db"
log "got RG public key (fingerprint on GET /status)"
kill "$(cat "${PIDDIR}/gateway-preboot.pid")" 2>/dev/null || true
rm -f "${PIDDIR}/gateway-preboot.pid"
# wait until the port is actually free again
for _ in $(seq 1 30); do
  port_open "${GATEWAY_ADDR##*:}" || break
  sleep 1
done
port_open "${GATEWAY_ADDR##*:}" \
  && fail "pre-boot gateway did not release ${GATEWAY_ADDR}" || true

# --- 3b. certauth (own port; generates ./ca-data on first boot) ---
# Pins are passed explicitly (never from .env defaults alone): BLS key from
# the pinned file above, RG key scraped from the pre-boot, strict CSR mode.
log "starting sagex-certauth on 127.0.0.1:${CERTAUTH_PORT}"
CHATSRV_PK_B64="$MLDSA65_PK_B64" CHATSRV_KID="$MLDSA65_KID" \
  RG_PK_B64="$RG_PK_B64" RG_ID="${RG_ID:-sagex-gateway-01}" REQUIRE_CHATSRV_TOKEN=true \
  HOST=127.0.0.1 PORT="$CERTAUTH_PORT" \
  nohup "${BINDIR}/sagex-certauth" \
  >"${LOGDIR}/certauth.log" 2>&1 &
echo "$!" >"${PIDDIR}/certauth.pid"

PUBFILE="${CA_KEY_DIR}/.pub"
log "waiting for CA key (${PUBFILE})"
i=0
while [[ ! -f "$PUBFILE" ]]; do
  i=$((i + 1))
  [[ "$i" -ge 180 ]] && fail "CA .pub never appeared (see logs/certauth.log)"
  sleep 1
done
wait_for_http "http://127.0.0.1:${CERTAUTH_PORT}/health" 60 \
  || fail "certauth never healthy (see logs/certauth.log)"
log "certauth healthy"

# --- 4. gateway (auto-wire CA public key into a runtime config) ---
log "extracting CA public key for gateway [auth]"
CA_PUB_B64="$(python3 - "$PUBFILE" <<'EOF'
import base64, struct, sys
data = open(sys.argv[1], 'rb').read()
magic, version, kem_len, dsa_len = struct.unpack_from('<IIII', data, 0)
assert (magic, version) == (0x00106E5A, 1), "not a sagex .pub file"
dsa = data[16 + kem_len:16 + kem_len + dsa_len]
assert len(dsa) == 1952, f"unexpected DSA key length: {len(dsa)}"
print(base64.b64encode(dsa).decode())
EOF
)" || fail "could not extract CA public key from $PUBFILE"
[[ -n "$CA_PUB_B64" ]] || fail "empty CA public key from $PUBFILE"

GW_TEMPLATE="crates/sagex-gateway/config/gateway.toml"
GW_CONFIG="${RUNDIR}/gateway.toml"
[[ -f "$GW_TEMPLATE" ]] || fail "missing gateway template: $GW_TEMPLATE"
CA_IDENTITY_ESC="${CA_IDENTITY}" GATEWAY_ADDR_ESC="${GATEWAY_ADDR}" CA_PUB_B64_ESC="${CA_PUB_B64}" \
python3 - "$GW_TEMPLATE" "$GW_CONFIG" <<'EOF'
import os, sys
section = None
out = []
for line in open(sys.argv[1]):
    s = line.strip()
    if s.startswith("[") and s.endswith("]"):
        section = s[1:-1]
    if section == "server" and line.startswith("listen = "):
        line = f'listen = "{os.environ["GATEWAY_ADDR_ESC"]}"\n'
    if section == "auth" and line.startswith("enabled = "):
        line = "enabled = true\n"
    if section == "auth" and line.startswith("ca_id = "):
        line = f'ca_id = "{os.environ["CA_IDENTITY_ESC"]}"\n'
    if section == "auth" and line.startswith("ca_pubkey_b64 = "):
        line = f'ca_pubkey_b64 = "{os.environ["CA_PUB_B64_ESC"]}"\n'
    out.append(line)
open(sys.argv[2], "w").writelines(out)
EOF
log "wrote ${GW_CONFIG} (auth pinned to CA '${CA_IDENTITY}')"

log "starting sagex-gateway on ${GATEWAY_ADDR}"
nohup "${BINDIR}/sagex-gateway" run --config "$GW_CONFIG" \
  >"${LOGDIR}/gateway.log" 2>&1 &
echo "$!" >"${PIDDIR}/gateway.pid"
wait_for_http "http://${GATEWAY_ADDR}/health" 60 \
  || fail "gateway never healthy (see logs/gateway.log)"
NODES="$(curl -sf -m 10 "http://${GATEWAY_ADDR}/status" \
  | python3 -c 'import sys,json; print(len(json.load(sys.stdin)["ledger"]))' || true)"
[[ "$NODES" == "4" ]] || fail "gateway sees ${NODES:-0}/4 ledger nodes (see logs/gateway.log)"
log "gateway healthy, sees 4/4 ledger nodes"

# --- 5. chatsrv (keeps 8080, the port sagex_ui points at) ---
# Pinned ML-DSA keys (shell-local vars, passed inline only to this process).
# Media backend: real MinIO when it booted, else explicit empty endpoint
# (in-memory fake) — never inherit a stale MINIO_ENDPOINT into a fake run.
CHAT_MONGODB_URI="mongodb://${MONGO_ROOT_USER}:${MONGO_ROOT_PASSWORD}@127.0.0.1:${MONGO_PORT}/chatdbx?authSource=admin"
if [[ "$MINIO_UP" == "true" ]]; then
  CHAT_MINIO_ENDPOINT="${MINIO_ENDPOINT:-http://127.0.0.1:9000}"
  log "starting sagex-chatsrv on ${CHAT_BIND} (pinned ML-DSA key, MinIO media)"
else
  CHAT_MINIO_ENDPOINT=""
  echo "WARN: chatsrv media endpoints use in-memory fake (MinIO unavailable)" >&2
  log "starting sagex-chatsrv on ${CHAT_BIND} (pinned ML-DSA key, fake media)"
fi
MLDSA65_SK_B64="$MLDSA65_SK_B64" MLDSA65_PK_B64="$MLDSA65_PK_B64" MLDSA65_KID="$MLDSA65_KID" \
  MINIO_ENDPOINT="$CHAT_MINIO_ENDPOINT" \
  BIND_ADDR="$CHAT_BIND" MONGODB_URI="$CHAT_MONGODB_URI" DB_NAME=chatdbx \
  nohup "${BINDIR}/sagex-chatsrv" \
  >"${LOGDIR}/chatsrv.log" 2>&1 &
echo "$!" >"${PIDDIR}/chatsrv.pid"
CHAT_PORT="${CHAT_BIND##*:}"
wait_for_http "http://127.0.0.1:${CHAT_PORT}/health" 60 \
  || fail "chatsrv never healthy (see logs/chatsrv.log)"
wait_for_http "http://127.0.0.1:${CHAT_PORT}/ready" 60 \
  || fail "chatsrv never ready (Mongo chatdbx unreachable? see logs/chatsrv.log)"
log "chatsrv healthy + ready"

# --- summary smoke ---
log "smoke:"
curl -sf -m 5 "http://127.0.0.1:${CHAT_PORT}/health" | head -c 120; echo
curl -sf -m 5 "http://127.0.0.1:${CERTAUTH_PORT}/health" | head -c 120; echo
curl -sf -m 5 "http://${GATEWAY_ADDR}/health"; echo
echo
echo "sagex backend up:"
echo "  chatsrv   http://${CHAT_BIND}  (UI points here)"
echo "  gateway   http://${GATEWAY_ADDR}  (auth: CA '${CA_IDENTITY}')"
echo "  certauth  http://127.0.0.1:${CERTAUTH_PORT}"
echo "  ledger    127.0.0.1:7000-7003   mongo 127.0.0.1:${MONGO_PORT}"
echo "logs: ${LOGDIR}/  pids: ${PIDDIR}/  stop: ./scripts/stop.sh"
