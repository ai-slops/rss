#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
mkdir -p var/recovery-review/runtime
source scripts/ubuntu-env.sh
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/recovery-review/runtime"
unset DISPLAY
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git merge-base --is-ancestor b2c6e22f3fc42206803cf1c5a72bb75321131489 HEAD
GIT_MASTER=1 git merge-base --is-ancestor 88e0cd7ceefe3e3ce2608a9510c189612bab2402 HEAD
{ date -u +%FT%TZ; GIT_MASTER=1 git rev-parse HEAD; cat /proc/sys/kernel/random/boot_id; uname -a; rustc --version; cargo --version; node --version; pkg-config --modversion gstreamer-1.0; ffmpeg -version; } > var/recovery-review/inventory.txt
: > var/recovery-review/results.tsv
failed=0; LAST_RESULT=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/recovery-review/$name.log" 2>&1 || result=$?
  LAST_RESULT=$result
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/recovery-review/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 16 "var/recovery-review/$name.log"; fi
}
gate debug-build cargo build --locked --workspace
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
gate release-build cargo build --locked --release --workspace
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
SUP="$ROOT/target/debug/live-production-supervisor"
gate recovery-unit "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production --lib source_recovery -- --nocapture
gate recovery-http "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production --test http_source_recovery -- --nocapture
gate program-content-unit "$SUP" --timeout-ms 30000 --grace-ms 5000 -- node --test scripts/target/harness/program-content-fence.test.mjs
gate engine-run-live "$SUP" --timeout-ms 180000 --grace-ms 10000 -- node scripts/target/recovery-engine-run.driver.mjs "$ROOT/var/recovery-review/engine-run-live"
gate aac-drain-matrix "$SUP" --timeout-ms 180000 --grace-ms 10000 -- node scripts/target/finite-aac-drain-matrix.mjs "$SUP" "$ROOT/var/recovery-review/aac-matrix"
gate native-controls env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=controls scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/recovery-review/native-controls" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 180
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
GIT_MASTER=1 git status --porcelain > var/recovery-review/source-status.txt
GIT_MASTER=1 git diff --exit-code 88e0cd7ceefe3e3ce2608a9510c189612bab2402 HEAD -- server/src media/src integrations
ps -eo pid,ppid,stat,comm > var/recovery-review/processes-after.txt
printf 'bounded_review_finished=%s failed=%s task_complete=false\n' "$(date -u +%FT%TZ)" "$failed"
exit "$failed"
