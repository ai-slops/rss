#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export LIVE_PRODUCTION_SMOKE_TIMEOUT_MS=180000
source "$PROJECT_ROOT/scripts/smoke-supervisor.sh"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
cargo build --locked --manifest-path "$PROJECT_ROOT/Cargo.toml" -p live-production -p live-production-supervisor
node --test "$PROJECT_ROOT/scripts/process-lifecycle.test.mjs" "$PROJECT_ROOT/scripts/native-lifecycle.test.mjs"
