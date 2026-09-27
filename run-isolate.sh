#!/usr/bin/env bash
# usage: ./run-isolated.sh /path/to/real/home/dir [/fake/path/inside]
REAL_DIR="${1:?need a real directory path}"
FAKE_PATH="${2:-/home/fakeuser}"

echo "$REAL_DIR"
# point this at wherever your actual build lives
BINARY_PATH="${BINARY_PATH:-$(pwd)/target/debug/sagex_test}"

mkdir -p "$REAL_DIR"

exec bwrap \
  --ro-bind /usr /usr --ro-bind /lib /lib --ro-bind /lib64 /lib64 --ro-bind /bin /bin \
  --ro-bind "$(dirname "$BINARY_PATH")" "$(dirname "$BINARY_PATH")" \
  --bind "$REAL_DIR" "$FAKE_PATH" \
  --setenv HOME "$FAKE_PATH" \
  --chdir "$FAKE_PATH" \
  --proc /proc --dev /dev \
  --unshare-all --share-net \
  "$BINARY_PATH"
