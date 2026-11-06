#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/recovery-review-caps
source scripts/ubuntu-env.sh
GIT_MASTER=1 git rev-parse HEAD > var/recovery-review-caps/source-head.txt
SUP="$ROOT/target/debug/live-production-supervisor"
failed=0
: > var/recovery-review-caps/results.tsv
for profile in original fixed; do
  diagnostic=0; [ "$profile" = original ] || diagnostic=1
  result=0
  LIVE_PRODUCTION_FIXED_CAPS_DIAGNOSTIC="$diagnostic" LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/recovery-review-caps/$profile" GST_DEBUG=matroskamux:5,splitmuxsink:5,x264enc:4 \
    "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test runtime_recording_contract -- --nocapture > "var/recovery-review-caps/$profile.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\n' "$profile" "$result" "$(date -u +%FT%TZ)" | tee -a var/recovery-review-caps/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 12 "var/recovery-review-caps/$profile.log"; fi
done
cargo fmt --all -- --check > var/recovery-review-caps/fmt.log 2>&1
cargo clippy --locked --workspace --all-targets -- -D warnings > var/recovery-review-caps/clippy.log 2>&1
GIT_MASTER=1 git status --porcelain > var/recovery-review-caps/source-status.txt
exit "$failed"
