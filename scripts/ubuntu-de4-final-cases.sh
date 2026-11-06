#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
RUN=${LIVE_NATIVE_RUN_NAME:-de4-native-05}
mkdir -p "var/$RUN/runtime"
source scripts/ubuntu-env.sh
export CARGO_TARGET_DIR="$HOME/review88/target" LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/$RUN/runtime"
unset DISPLAY LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_OMIT LIVE_PRODUCTION_OBS_METER_DIAGNOSTIC
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git diff --exit-code de4b80058871f4ec5f138ddb25cb68d05c5bc718 HEAD -- server/src media/src integrations
GIT_MASTER=1 git rev-parse HEAD > "var/$RUN/source-head.txt"
: > "var/$RUN/results.tsv"
failed=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/$RUN/$name.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a "var/$RUN/results.tsv"
  if [ "$result" != 0 ]; then failed=1; fi
}
recovery() {
  scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/de4-plugin-stage" --evidence-dir "$ROOT/var/$RUN/$1" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
}
export RUN
export -f recovery
gate recovery-full recovery recovery-full
gate recovery-fences env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=fences bash -c 'recovery recovery-fences'
gate recovery-budget env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=budget bash -c 'recovery recovery-budget'
gate recovery-controls env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=controls bash -c 'recovery recovery-controls'
export LIVE_PRODUCTION_REHEARSAL_DRIVER=1 LIVE_PRODUCTION_REHEARSAL_ROOT="$ROOT"
export LIVE_PRODUCTION_TEST_SERVER="$ROOT/target/release/live-production" LIVE_PRODUCTION_TEST_SUPERVISOR="$ROOT/target/release/live-production-supervisor" LIVE_PRODUCTION_TEST_WEB_DIST="$ROOT/web/dist"
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1 LIVE_PRODUCTION_RECOVERY_ROOT="$ROOT"
export LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR" LIVE_PRODUCTION_RECOVERY_SERVER="$LIVE_PRODUCTION_TEST_SERVER" LIVE_PRODUCTION_RECOVERY_WEB_DIST="$ROOT/web/dist"
export FFMPEG=/usr/bin/ffmpeg FFPROBE=/usr/bin/ffprobe
gate target-tests target/debug/live-production-supervisor --timeout-ms 600000 --grace-ms 10000 -- node --test --test-concurrency=1 scripts/target/*.test.mjs scripts/target/harness/*.test.mjs
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
GIT_MASTER=1 git status --porcelain > "var/$RUN/source-status.txt"
sha256sum var/de4-plugin-stage/bin/64bit/obs-live-production.so target/release/live-production target/release/live-production-supervisor > "var/$RUN/binary-hashes.txt"
ps -eo pid,ppid,stat,comm > "var/$RUN/processes-after.txt"
exit "$failed"
