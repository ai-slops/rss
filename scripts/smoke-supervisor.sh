#!/bin/sh

. "$PROJECT_ROOT/scripts/env.sh"
if [ "${LIVE_PRODUCTION_SMOKE_SCOPE:-}" != "$0" ]; then
  CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} cargo build --locked \
    --manifest-path "$PROJECT_ROOT/Cargo.toml" -q -p live-production-supervisor
  exec "$PROJECT_ROOT/target/debug/live-production-supervisor" \
    --timeout-ms "${LIVE_PRODUCTION_SMOKE_TIMEOUT_MS:-240000}" --grace-ms 10000 \
    -- env LIVE_PRODUCTION_SMOKE_SCOPE="$0" bash "$0" "$@"
fi

stop_owned_process() {
  owned_pid=$1
  if [ -z "$owned_pid" ]; then return; fi
  kill -TERM "$owned_pid" 2>/dev/null || true
  count=0
  while kill -0 "$owned_pid" 2>/dev/null && [ "$count" -lt 100 ]; do
    sleep 0.1
    count=$((count + 1))
  done
  if kill -0 "$owned_pid" 2>/dev/null; then kill -KILL "$owned_pid" 2>/dev/null || true; fi
  wait "$owned_pid" 2>/dev/null || true
}
