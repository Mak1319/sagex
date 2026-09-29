#!/usr/bin/env bash
# Stop the sagex backend stack started by scripts/start.sh.
#
# Usage:
#   ./scripts/stop.sh        # stop processes, keep mongodb running (data preserved)
#   ./scripts/stop.sh --down # also `docker compose down` mongodb (volume data preserved)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PIDDIR=".pids"

stop_one() { # name
  local pidfile="${PIDDIR}/$1.pid"
  [[ -f "$pidfile" ]] || return 0
  local pid
  pid="$(cat "$pidfile")"
  if kill -0 "$pid" 2>/dev/null; then
    echo "stopping $1 (pid $pid)"
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 10); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 1
    done
    if kill -0 "$pid" 2>/dev/null; then
      echo "WARN: $1 did not exit, killing -9"
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
  rm -f "$pidfile"
}

# Reverse boot order.
stop_one chatsrv
stop_one gateway
stop_one certauth
for n in 0 1 2 3; do
  stop_one "ledger-node${n}"
done

if [[ "${1:-}" == "--down" ]]; then
  echo "stopping mongodb (docker compose down; volume data preserved)"
  docker compose down
else
  echo "mongodb left running (data preserved); use './scripts/stop.sh --down' to stop it"
fi
echo "stopped"
