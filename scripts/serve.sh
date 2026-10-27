#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
source "$ROOT/scripts/env.sh"
if [[ ! -x "$ROOT/target/debug/live-production" ]]; then
  printf '%s\n' 'Build the application with scripts/dev.sh before starting the managed service.' >&2
  exit 1
fi
cd "$ROOT"
exec "$ROOT/target/debug/live-production-supervisor" -- "$ROOT/target/debug/live-production"
