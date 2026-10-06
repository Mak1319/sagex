#!/bin/sh
set -eu

env_path="${1:?usage: run-isolate.sh environments/sagex-nodeblock [service args...]}"
shift
repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
env_dir="$(realpath -m -- "$env_path")"
binary_path="${BINARY_PATH:-$repo_root/target/debug/sagex-nodeblock}"
binary_path="$(realpath -- "$binary_path")"

mkdir -p -- "$env_dir"
exec bwrap \
    --die-with-parent \
    --unshare-pid --unshare-uts --unshare-ipc --unshare-cgroup \
    --share-net \
    --ro-bind /usr /usr --ro-bind /etc /etc \
    --symlink usr/bin /bin --symlink usr/lib /lib \
    --symlink usr/lib64 /lib64 --symlink usr/sbin /sbin \
    --proc /proc --dev /dev --tmpfs /tmp \
    --dir /opt --dir /service \
    --ro-bind "$binary_path" /opt/sagex-nodeblock \
    --bind "$env_dir" /service \
    --setenv HOME /service \
    --setenv PATH /usr/local/bin:/usr/bin:/bin \
    --chdir /service \
    /opt/sagex-nodeblock "$@"
