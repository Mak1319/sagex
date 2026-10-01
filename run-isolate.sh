#!/usr/bin/env bash
# run a compiled-service binary isolated with bwrap.
# usage: BINARY_PATH=target/debug/<svc> ./run-isolate.sh environments/<svc> [args...]
#    or: ./run-isolate.sh environments/<svc> target/debug/<svc> [args...]
# env dir is mounted as HOME; system dirs are read-only; namespaces unshared.
#
# GUI services (needs a window): GUI=1 adds the minimum host binds for
# Wayland/X11 + GPU + fonts + session bus. This weakens isolation to
# roughly flatpak level — accepted price for a window, headless services
# stay fully locked down (default GUI=0).
# usage: GUI=1 BINARY_PATH=target/debug/sagex-desk ./run-isolate.sh environments/desk
REAL_DIR="${1:?need an environment directory path (under environments/)}"
if [ -n "${2:-}" ] && [ -f "${2:-}" ]; then
  BINARY_PATH="$2"
  shift 2
else
  # point this at wherever your actual build lives
  BINARY_PATH="${BINARY_PATH:-$(pwd)/target/debug/sagex_test}"
  shift
fi
FAKE_PATH="${FAKE_PATH:-/home/fakeuser}"
FAKE_RUN="$FAKE_PATH/.run"

echo "$REAL_DIR"

mkdir -p "$REAL_DIR"

GUI_BINDS=()
GUI_ENVS=()
if [ "${GUI:-0}" = "1" ]; then
  HOST_RT="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
  mkdir -p "$REAL_DIR/.run"
  # Wayland socket (per-file binds; dir itself stays host-private).
  if [ -S "$HOST_RT/wayland-0" ]; then
    GUI_BINDS+=(--bind "$HOST_RT/wayland-0" "$FAKE_RUN/wayland-0")
  fi
  for i in 1 2 3; do
    if [ -S "$HOST_RT/wayland-$i" ]; then
      GUI_BINDS+=(--bind "$HOST_RT/wayland-$i" "$FAKE_RUN/wayland-$i")
    fi
  done
  # Session bus (secret-service/keyring, notifications).
  if [ -S "$HOST_RT/bus" ]; then
    GUI_BINDS+=(--bind "$HOST_RT/bus" "$FAKE_RUN/bus")
    GUI_ENVS+=(--setenv DBUS_SESSION_BUS_ADDRESS "unix:path=$FAKE_RUN/bus")
  fi
  # X11 fallback dir (primary path is the abstract socket via --share-net).
  if [ -d /tmp/.X11-unix ]; then
    GUI_BINDS+=(--ro-bind /tmp/.X11-unix /tmp/.X11-unix)
  fi
  # Xauthority for the X11 fallback path (primary is Wayland).
  if [ -n "${XAUTHORITY:-}" ] && [ -f "$XAUTHORITY" ]; then
    GUI_BINDS+=(--ro-bind "$XAUTHORITY" "$FAKE_RUN/Xauthority")
    GUI_ENVS+=(--setenv XAUTHORITY "$FAKE_RUN/Xauthority")
  fi
  GUI_BINDS+=(--ro-bind /etc/fonts /etc/fonts)
  if [ -d /dev/dri ]; then
    GUI_BINDS+=(--dev-bind /dev/dri /dev/dri)
  fi
  GUI_ENVS+=(
    --setenv XDG_RUNTIME_DIR "$FAKE_RUN"
    --setenv WAYLAND_DISPLAY "${WAYLAND_DISPLAY:-wayland-0}"
    --setenv DISPLAY "${DISPLAY:-:0}"
    --setenv XDG_SESSION_TYPE wayland
    --setenv LIBGL_ALWAYS_SOFTWARE 1
  )
fi

exec bwrap \
  --ro-bind /usr /usr --ro-bind /lib /lib --ro-bind /lib64 /lib64 --symlink usr/bin /bin \
  --ro-bind /etc/resolv.conf /etc/resolv.conf --ro-bind /etc/hosts /etc/hosts \
  --ro-bind "$(dirname "$BINARY_PATH")" "$(dirname "$BINARY_PATH")" \
  --bind "$REAL_DIR" "$FAKE_PATH" \
  --setenv HOME "$FAKE_PATH" \
  "${GUI_BINDS[@]}" "${GUI_ENVS[@]}" \
  --chdir "$FAKE_PATH" \
  --proc /proc --dev /dev \
  --unshare-all --share-net \
  "$BINARY_PATH" "$@"
