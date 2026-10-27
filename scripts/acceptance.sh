#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PROJECT_ROOT=$ROOT
export LIVE_PRODUCTION_SMOKE_TIMEOUT_MS=${LIVE_PRODUCTION_SMOKE_TIMEOUT_MS:-1200000}
. "$ROOT/scripts/smoke-supervisor.sh"
source "$ROOT/scripts/env.sh"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
cd "$ROOT"
mkdir -p var/acceptance
REPORT="$ROOT/var/acceptance/latest.txt"
exec > >(tee "$REPORT") 2>&1
printf 'Live Production acceptance — %s\n' "$(date -u +%FT%TZ)"
printf 'Commit: %s\n' "$(GIT_MASTER=1 git rev-parse HEAD)"
./scripts/lifecycle-acceptance.sh
node scripts/size-check.mjs
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
npm --prefix web run check
npm --prefix web test
npm --prefix web run build
npm --prefix web audit --audit-level=moderate
if [[ ${1:-} == --quick ]]; then
  printf '%s\n' 'Quick checks passed. Full media, browser and extension gates were NOT executed.'
  exit 0
fi
./scripts/media-stage-a.sh
./scripts/media-network-gates.sh
for gate in output-acceptance browser-acceptance editor-acceptance; do
  if [[ ! -x "scripts/$gate.sh" ]]; then
    printf 'REQUIRED GATE UNAVAILABLE: scripts/%s.sh\n' "$gate" >&2
    exit 1
  fi
  "./scripts/$gate.sh"
done
printf '%s\n' 'Controlled software acceptance passed. External OBS/platform/laptop/hardware gates remain separately unverified.'
