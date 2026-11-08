#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/recovery-review-format
source scripts/ubuntu-env.sh
GIT_MASTER=1 git rev-parse HEAD > var/recovery-review-format/source-head.txt
result=0
LIVE_PRODUCTION_FIXED_CAPS_DIAGNOSTIC=format LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/recovery-review-format/media" GST_DEBUG=matroskamux:5,splitmuxsink:5,x264enc:4 \
  target/debug/live-production-supervisor --timeout-ms 30000 --grace-ms 5000 -- cargo test --locked -p live-production-media --test runtime_recording_contract -- --nocapture > var/recovery-review-format/format.log 2>&1 || result=$?
printf 'format-only\t%s\t%s\n' "$result" "$(date -u +%FT%TZ)" | tee var/recovery-review-format/results.tsv
cargo fmt --all -- --check > var/recovery-review-format/fmt.log 2>&1
exit "$result"
