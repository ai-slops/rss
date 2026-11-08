#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/recovery-review-02
source scripts/ubuntu-env.sh
GIT_MASTER=1 git rev-parse HEAD > var/recovery-review-02/source-head.txt
SUP="$ROOT/target/debug/live-production-supervisor"
failed=0
: > var/recovery-review-02/results.tsv
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/recovery-review-02/$name.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/recovery-review-02/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 16 "var/recovery-review-02/$name.log"; fi
}
gate engine-run-live "$SUP" --timeout-ms 180000 --grace-ms 10000 -- node scripts/target/recovery-engine-run.driver.mjs "$ROOT/var/recovery-review-02/engine-run-live"
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_ROOT="$ROOT" LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$ROOT/target/release/live-production-supervisor"
gate finite-calibration "$SUP" --timeout-ms 180000 --grace-ms 10000 -- node --test scripts/target/recovery-local.driver.test.mjs
gate program-recording-trace env LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/recovery-review-02/media" GST_DEBUG=matroskamux:5,splitmuxsink:5,x264enc:4 "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test runtime_recording_contract -- --nocapture
gate program-fence-unit "$SUP" --timeout-ms 30000 --grace-ms 5000 -- node --test scripts/target/harness/program-content-fence.test.mjs
gate fmt cargo fmt --all -- --check
GIT_MASTER=1 git status --porcelain > var/recovery-review-02/source-status.txt
ps -eo pid,ppid,stat,comm > var/recovery-review-02/processes-after.txt
printf 'bounded_second_finished=%s failed=%s task_complete=false\n' "$(date -u +%FT%TZ)" "$failed"
exit "$failed"
