#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export LIVE_PRODUCTION_SMOKE_TIMEOUT_MS=150000
source "$PROJECT_ROOT/scripts/smoke-supervisor.sh"
umask 077
mkdir -p "$PROJECT_ROOT/var"
OUTPUT_DIR=${1:-$(mktemp -d "$PROJECT_ROOT/var/browser-acceptance.XXXXXX")}
mkdir -p "$OUTPUT_DIR/state"
BIND="127.0.0.1:$(node "$PROJECT_ROOT/scripts/free-port.mjs" tcp)"
npm --prefix "$PROJECT_ROOT/web" run build
CARGO_BUILD_JOBS=2 cargo build --locked --manifest-path "$PROJECT_ROOT/Cargo.toml" -q -p live-production
server_pid=
cleanup() { stop_owned_process "${server_pid:-}"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
LIVE_PRODUCTION_BIND="$BIND" LIVE_PRODUCTION_STATE_DIR="$OUTPUT_DIR/state" \
  LIVE_PRODUCTION_PASSWORD=test-operator-password LIVE_PRODUCTION_SECURE_COOKIE=false \
  LIVE_PRODUCTION_WEB_DIST="$PROJECT_ROOT/web/dist" \
  "$PROJECT_ROOT/target/debug/live-production" >"$OUTPUT_DIR/server.log" 2>&1 &
server_pid=$!
for attempt in $(seq 1 80); do
  if curl --silent --fail --max-time 1 "http://$BIND/" >/dev/null; then break; fi
  if ! kill -0 "$server_pid" 2>/dev/null; then printf '%s\n' "Browser fixture exited; inspect $OUTPUT_DIR/server.log" >&2; exit 1; fi
  sleep 0.1
done
LIVE_PRODUCTION_URL="http://$BIND" LIVE_PRODUCTION_PASSWORD=test-operator-password \
  BROWSER_EVIDENCE_DIR="$OUTPUT_DIR" node "$PROJECT_ROOT/scripts/browser-acceptance.mjs"
printf 'browser_gate=passed\nartifacts=%s\n' "$OUTPUT_DIR"
