#!/usr/bin/env bash
# Build all sagex backend binaries.
#
# Usage:
#   ./scripts/build.sh            # debug build (default)
#   ./scripts/build.sh --release  # release build (also picked up by start.sh via PROFILE=release)
#
# Extra args are passed through to cargo (e.g. ./scripts/build.sh --release -j8).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PACKAGES=(
  sagex-ledger
  sagex-gateway
  sagex-certauth
  sagex-chatsrv
)

# Binary name per package (ledger's binary differs from its package name).
BINARIES=(
  sagex-ledger-node
  sagex-gateway
  sagex-certauth
  sagex-chatsrv
)

PROFILE="debug"
CARGO_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --release) PROFILE="release"; CARGO_ARGS+=("--release") ;;
    *) CARGO_ARGS+=("$arg") ;;
  esac
done

echo "==> cargo build (${PROFILE}): ${PACKAGES[*]}"
cargo build "${CARGO_ARGS[@]}" -p sagex-ledger -p sagex-gateway -p sagex-certauth -p sagex-chatsrv

echo "==> verifying binaries in target/${PROFILE}/"
missing=0
for bin in "${BINARIES[@]}"; do
  if [[ -x "target/${PROFILE}/${bin}" ]]; then
    echo "  ok: ${bin}"
  else
    echo "  MISSING: ${bin}" >&2
    missing=1
  fi
done

if [[ "$missing" -ne 0 ]]; then
  echo "error: some binaries are missing after build" >&2
  exit 1
fi
echo "build ok (${PROFILE})"
