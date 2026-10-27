#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [[ ! -x "$ROOT/.tools/sysroot/usr/bin/rustc" ]]; then
  "$ROOT/scripts/bootstrap.sh"
fi
source "$ROOT/scripts/env.sh"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
export LIVE_PRODUCTION_SECURE_COOKIE=${LIVE_PRODUCTION_SECURE_COOKIE:-false}
npm --prefix "$ROOT/web" ci --no-fund --no-audit
npm --prefix "$ROOT/web" run build
cargo build --locked --manifest-path "$ROOT/Cargo.toml" -p live-production -p live-production-supervisor
cd "$ROOT"
exec "$ROOT/scripts/serve.sh"
