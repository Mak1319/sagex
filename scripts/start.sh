#!/usr/bin/env bash
# Start the full sagex backend stack (no desktop UI).
#
# Boot order (forced by dependencies):
#   1. mongodb (docker compose, both Mongo DBs ride it: chatdbx, sagexcadb)
#   2. 4x sagex-ledger-node (7000-7003, PBFT mesh)
#   3. sagex-certauth (127.0.0.1:8082; generates ./ca-data on first boot)
#   4. sagex-gateway (127.0.0.1:8081; config auto-wired with the CA public key)
#   5. sagex-chatsrv (0.0.0.0:8080; the port sagex_ui points at)
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

# --- port guard: chatsrv 8080, gateway 8081, certauth 8082 must all be free ---
port_open() { timeout 1 bash -c "echo > /dev/tcp/127.0.0.1/$1" 2>/dev/null; }
for p in "$MONGO_PORT" 7000 7001 7002 7003 8080 8081 "$CERTAUTH_PORT"; do
  if port_open "$p"; then
    fail "port $p is already in use — stop whatever holds it first (see scripts/stop.sh)"
  fi
done

# --- binaries (built by scripts/build.sh) ---
for bin in sagex-ledger-node sagex-gateway sagex-certauth sagex-chatsrv; do
  [[ -x "${BINDIR}/${bin}" ]] \
    || fail "missing ${BINDIR}/${bin} — run ./scripts/build.sh first"
done

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

# --- 3. certauth (own port; generates ./ca-data on first boot) ---
log "starting sagex-certauth on 127.0.0.1:${CERTAUTH_PORT}"
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
sed -e "s|^listen = .*|listen = \"${GATEWAY_ADDR}\"|" \
    -e "s|^enabled = .*|enabled = true|" \
    -e "s|^ca_id = .*|ca_id = \"${CA_IDENTITY}\"|" \
    -e "s|^ca_pubkey_b64 = .*|ca_pubkey_b64 = \"${CA_PUB_B64}\"|" \
    "$GW_TEMPLATE" >"$GW_CONFIG"
log "wrote ${GW_CONFIG} (auth pinned to CA '${CA_IDENTITY}')"

log "starting sagex-gateway on ${GATEWAY_ADDR}"
nohup "${BINDIR}/sagex-gateway" run --config "$GW_CONFIG" \
  >"${LOGDIR}/gateway.log" 2>&1 &
echo "$!" >"${PIDDIR}/gateway.pid"
wait_for_http "http://${GATEWAY_ADDR}/health" 60 \
  || fail "gateway never healthy (see logs/gateway.log)"
NODES="$(curl -sf -m 10 "http://${GATEWAY_ADDR}/status" \
  | python3 -c 'import sys,json; print(len(json.load(sys.stdin)["ledger"]))')"
[[ "$NODES" == "4" ]] || fail "gateway sees ${NODES:-0}/4 ledger nodes (see logs/gateway.log)"
log "gateway healthy, sees 4/4 ledger nodes"

# --- 5. chatsrv (keeps 8080, the port sagex_ui points at) ---
CHAT_MONGODB_URI="mongodb://${MONGO_ROOT_USER}:${MONGO_ROOT_PASSWORD}@127.0.0.1:${MONGO_PORT}/chatdbx?authSource=admin"
log "starting sagex-chatsrv on ${CHAT_BIND}"
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
