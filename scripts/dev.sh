#!/usr/bin/env bash
# Development loop: runs the development profile (ALMAHRYA_PROFILE=dev) and rebuilds on
# save. A change in a backend crate or in sidecars/ restarts almahryad; a change in the
# desktop crates rebuilds and restarts the app. Ctrl-C stops both loops and everything
# they started.
#
# The development profile has its own data directory and port, so this never touches
# production data or the production launchd agent. The dev API token lives in a file in
# the dev data directory (ALMAHRYA_TOKEN_FILE, mode 0600, directory 0700), so neither loop
# ever reads or writes the Keychain. The daemon creates the token on its first start; the
# app only reads it.
#
# It refuses to start when ALMAHRYA_DATA_DIR, ALMAHRYA_CONFIG or ALMAHRYA_TOKEN_FILE is
# already set: an inherited override could point the dev profile at production state.
#
#   scripts/dev.sh           run the loops (needs watchexec)
#   scripts/dev.sh --print   print the commands it would run, then exit
set -euo pipefail

usage() {
  echo "usage: scripts/dev.sh [--print]"
}

PRINT=0
case "${1:-}" in
  "") ;;
  --print) PRINT=1 ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
if [ "$#" -gt 1 ]; then
  usage >&2
  exit 2
fi

for var in ALMAHRYA_DATA_DIR ALMAHRYA_CONFIG ALMAHRYA_TOKEN_FILE; do
  if [ -n "${!var+x}" ]; then
    echo "dev.sh: $var is set in the environment; unset it first, so the development" >&2
    echo "profile cannot open another profile's files." >&2
    exit 1
  fi
done
if [ -z "${HOME:-}" ]; then
  echo "dev.sh: HOME is not set" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
export ALMAHRYA_PROFILE=dev
# The default dev data directory (no ALMAHRYA_DATA_DIR, checked above), and the token in it.
DEV_DATA_DIR="$HOME/Library/Application Support/Almahrya-Dev"
export ALMAHRYA_TOKEN_FILE="$DEV_DATA_DIR/api-token"

# Watch only directories that exist; watchexec refuses a missing path.
watch_args() {
  local dir
  for dir in "$@"; do
    if [ -d "$dir" ]; then
      printf '%s\n' "--watch" "$dir"
    fi
  done
}

BACKEND_DIRS="crates/core crates/api-types crates/store crates/connectors crates/runtimes
crates/readpath crates/engine crates/api crates/daemon crates/local sidecars"
APP_DIRS="crates/ui-kit crates/app crates/client crates/local"

COMMON=(--restart --stop-signal SIGTERM --stop-timeout 5s --debounce 300ms
  --emit-events-to none
  --ignore 'target/**' --ignore '.git/**' --ignore '.superpowers/**' --ignore 'docs/**')

BACKEND_WATCH=()
for arg in $(watch_args $BACKEND_DIRS); do BACKEND_WATCH+=("$arg"); done
APP_WATCH=()
for arg in $(watch_args $APP_DIRS); do APP_WATCH+=("$arg"); done

BACKEND_CMD=(watchexec "${COMMON[@]}" "${BACKEND_WATCH[@]}" --
  cargo run --quiet -p almahrya-daemon --bin almahryad)
APP_CMD=(watchexec "${COMMON[@]}" "${APP_WATCH[@]}" --
  cargo run --quiet -p almahrya-app --bin almahrya)

if [ "$PRINT" -eq 1 ]; then
  echo "ALMAHRYA_PROFILE=$ALMAHRYA_PROFILE"
  echo "ALMAHRYA_TOKEN_FILE=$ALMAHRYA_TOKEN_FILE"
  printf 'backend:'
  printf ' %q' "${BACKEND_CMD[@]}"
  printf '\napp: ALMAHRYA_DAEMON_BIN=<stub that exits 3>'
  printf ' %q' "${APP_CMD[@]}"
  printf '\n'
  exit 0
fi

if ! command -v watchexec >/dev/null 2>&1; then
  echo "dev.sh needs watchexec. Install it with:" >&2
  echo "  brew install watchexec" >&2
  exit 1
fi

# Owner-only, like the daemon makes it; an existing token file stays private too.
mkdir -p "$DEV_DATA_DIR"
chmod 700 "$DEV_DATA_DIR"
if [ -e "$ALMAHRYA_TOKEN_FILE" ]; then
  chmod 600 "$ALMAHRYA_TOKEN_FILE"
fi

# The app starts a daemon itself when none answers. Here the backend loop owns the
# daemon, so the app gets a stand-in that reports "already running" (exit 3) and keeps
# waiting; otherwise a daemon started by the app would escape the loop and outlive it.
STUB_DIR="$(mktemp -d "${TMPDIR:-/private/tmp}/almahrya-dev.XXXXXX")"
printf '#!/bin/sh\nexit 3\n' >"$STUB_DIR/almahryad"
chmod 755 "$STUB_DIR/almahryad"

WATCHERS=""
CLEANED=0

# Every process below `pid`, children first in the output order of a depth-first walk.
descendants() {
  local child
  for child in $(pgrep -P "$1" 2>/dev/null || true); do
    echo "$child"
    descendants "$child"
  done
}

alive_any() {
  local pid
  for pid in "$@"; do
    if kill -0 "$pid" 2>/dev/null; then
      return 0
    fi
  done
  return 1
}

cleanup() {
  if [ "$CLEANED" -eq 1 ]; then
    return
  fi
  CLEANED=1
  trap - INT TERM
  local pids="" watcher pid i
  for watcher in $WATCHERS; do
    pids="$pids $(descendants "$watcher")"
  done
  echo "dev.sh: stopping" >&2
  # watchexec stops its command (SIGTERM, then SIGKILL after the stop timeout).
  for watcher in $WATCHERS; do
    kill -TERM "$watcher" 2>/dev/null || true
  done
  for i in $(seq 1 70); do
    # shellcheck disable=SC2086
    alive_any $WATCHERS $pids || break
    sleep 0.1
  done
  # Anything still running was started by the loops; make sure it ends.
  for pid in $pids; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  sleep 0.5
  for pid in $WATCHERS $pids; do
    kill -KILL "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  rm -rf "$STUB_DIR"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

echo "dev.sh: development profile; Ctrl-C stops everything" >&2
"${BACKEND_CMD[@]}" &
WATCHERS="$!"
ALMAHRYA_DAEMON_BIN="$STUB_DIR/almahryad" "${APP_CMD[@]}" &
WATCHERS="$WATCHERS $!"

# If either loop ends on its own, stop the other one too.
while true; do
  for watcher in $WATCHERS; do
    if ! kill -0 "$watcher" 2>/dev/null; then
      echo "dev.sh: a watch loop exited" >&2
      exit 1
    fi
  done
  sleep 1
done
