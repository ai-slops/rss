#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/recovery-review-i420
source scripts/ubuntu-env.sh
GIT_MASTER=1 git rev-parse HEAD > var/recovery-review-i420/source-head.txt
SUP="$ROOT/target/debug/live-production-supervisor"
failed=0
: > var/recovery-review-i420/results.tsv
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/recovery-review-i420/$name.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/recovery-review-i420/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 14 "var/recovery-review-i420/$name.log"; fi
}
gate program-recording env LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/recovery-review-i420/media" "$SUP" --timeout-ms 60000 --grace-ms 5000 -- cargo test --locked -p live-production-media --test runtime_recording_contract -- --nocapture
gate drift-rss env LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/recovery-review-i420/media" "$SUP" --timeout-ms 300000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test srt_drift_endurance -- --nocapture
gate clean-provenance env LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR="$ROOT/var/recovery-review-i420/provenance" GST_DEBUG=matroskamux:5,splitmuxsink:5,intervideosrc:5 "$SUP" --timeout-ms 60000 --grace-ms 5000 -- cargo test --locked -p live-production --test editor_clean_provenance -- --nocapture
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
GIT_MASTER=1 git diff --exit-code 88e0cd7ceefe3e3ce2608a9510c189612bab2402 HEAD -- server/src media/src integrations
GIT_MASTER=1 git status --porcelain > var/recovery-review-i420/source-status.txt
exit "$failed"
