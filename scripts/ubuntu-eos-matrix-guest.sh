#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/de4-eos-matrix
source scripts/ubuntu-env.sh
export CARGO_TARGET_DIR="$HOME/review88/target"
unset GST_DEBUG GST_DEBUG_FILE LIVE_PRODUCTION_EOS_OBSERVE
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git diff --exit-code de4b80058871f4ec5f138ddb25cb68d05c5bc718 HEAD -- server/src media/src integrations
GIT_MASTER=1 git rev-parse HEAD > var/de4-eos-matrix/source-head.txt
{ date -u +%FT%TZ; uname -a; nproc; rustc --version; cargo --version; node --version; pkg-config --modversion gstreamer-1.0; } > var/de4-eos-matrix/inventory.txt
ps -eo pid,ppid,stat,comm > var/de4-eos-matrix/processes-before.txt
cargo test --locked -p live-production-media --test srt_drift_endurance --no-run > var/de4-eos-matrix/build.log 2>&1
cargo test --locked -p live-production-media --test srt_drift_endurance drift_support::eos::tests:: -- --nocapture > var/de4-eos-matrix/observer-tests.log 2>&1
node --test scripts/target/harness/eos-media-evidence.test.mjs > var/de4-eos-matrix/evidence-tests.log 2>&1
result=0
target/debug/live-production-supervisor --timeout-ms 1450000 --grace-ms 10000 -- node scripts/target/eos-soak-matrix.mjs "$ROOT/target/debug/live-production-supervisor" "$ROOT/var/de4-eos-matrix/cases" > var/de4-eos-matrix/matrix.log 2>&1 || result=$?
printf 'predeclared-eos-matrix\t%s\t%s\n' "$result" "$(date -u +%FT%TZ)" > var/de4-eos-matrix/results.tsv
cargo fmt --all -- --check > var/de4-eos-matrix/fmt.log 2>&1
cargo clippy --locked --workspace --all-targets -- -D warnings > var/de4-eos-matrix/clippy.log 2>&1
GIT_MASTER=1 git status --porcelain > var/de4-eos-matrix/source-status.txt
ps -eo pid,ppid,stat,comm > var/de4-eos-matrix/processes-after.txt
exit "$result"
