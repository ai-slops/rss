#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/output-browser-gate}
BIND=${OUTPUT_BROWSER_BIND:-127.0.0.1:3096}

. "$PROJECT_ROOT/scripts/env.sh"
mkdir -p "$OUTPUT_DIR/state"
(cd "$PROJECT_ROOT/web" && npm run build >/dev/null)
CARGO_BUILD_JOBS=3 cargo build -q -p live-production

cleanup() {
  kill "${server_pid:-}" 2>/dev/null || true
  wait "${server_pid:-}" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

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
wait "$server_pid"
server_pid=
printf '%s\n' "artifacts=$OUTPUT_DIR"
