#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$PROJECT_ROOT/scripts/smoke-supervisor.sh"
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/output-browser-gate}
BIND=${OUTPUT_BROWSER_BIND:-127.0.0.1:3096}

. "$PROJECT_ROOT/scripts/env.sh"
case "$OUTPUT_DIR" in
  "$PROJECT_ROOT"/var/*|var/*) ;;
  *) printf '%s\n' "browser evidence must stay under $PROJECT_ROOT/var" >&2; exit 1 ;;
esac
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR/state"
test -f "$PROJECT_ROOT/web/dist/index.html"
CARGO_BUILD_JOBS=3 cargo build -q -p live-production

cleanup() {
  stop_owned_process "${server_pid:-}"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

LIVE_PRODUCTION_BIND=$BIND \
LIVE_PRODUCTION_STATE_DIR="$OUTPUT_DIR/state" \
LIVE_PRODUCTION_PASSWORD=output-browser-password \
LIVE_PRODUCTION_SECURE_COOKIE=false \
LIVE_PRODUCTION_WEB_DIST="$PROJECT_ROOT/web/dist" \
"$PROJECT_ROOT/target/debug/live-production" >"$OUTPUT_DIR/server.log" 2>&1 &
server_pid=$!

attempt=0
until curl -sS --fail "http://$BIND/" >/dev/null 2>&1; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 60 ]; then
    printf '%s\n' "server did not become ready" >&2
    exit 1
  fi
  sleep 0.25
done

LIVE_PRODUCTION_URL="http://$BIND" \
LIVE_PRODUCTION_PASSWORD=output-browser-password \
OUTPUT_BROWSER_EVIDENCE="$OUTPUT_DIR" \
node "$PROJECT_ROOT/scripts/output-browser-gate.mjs"

kill -TERM "$server_pid"
attempt=0
while kill -0 "$server_pid" 2>/dev/null; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 40 ]; then
    kill -KILL "$server_pid" 2>/dev/null || true
    break
  fi
  sleep 0.25
done
wait "$server_pid" 2>/dev/null || true
server_pid=
printf '%s\n' "artifacts=$OUTPUT_DIR"
