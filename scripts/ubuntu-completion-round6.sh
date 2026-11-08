#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
mkdir -p var/completion-06
source scripts/ubuntu-env.sh
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/completion/runtime"
unset DISPLAY
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git rev-parse HEAD > var/completion-06/source-head.txt
SUP="$ROOT/target/debug/live-production-supervisor"
: > var/completion-06/results.tsv
failed=0; LAST_RESULT=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/completion-06/$name.log" 2>&1 || result=$?
  LAST_RESULT=$result
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/completion-06/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 18 "var/completion-06/$name.log"; fi
}
recovery() {
  scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/$1" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
}
export -f recovery
gate cmake-build cmake --build var/obs-plugin-build --parallel 2
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
gate ctest env QT_QPA_PLATFORM=offscreen timeout 180 ctest --test-dir var/obs-plugin-build --output-on-failure
gate cmake-install cmake --install var/obs-plugin-build --prefix "$ROOT/var/obs-plugin-stage"
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
gate obs-rehearsal scripts/target/rehearsal.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/obs-completion-05" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --obs-binary /usr/bin/obs --deadline-seconds 120
gate recovery-full recovery recovery-completion-12
gate recovery-fences env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=fences bash -c 'recovery recovery-completion-13'
gate recovery-budget env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=budget bash -c 'recovery recovery-completion-14'
gate recovery-controls env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=controls bash -c 'recovery recovery-completion-15'
export LIVE_PRODUCTION_REHEARSAL_DRIVER=1 LIVE_PRODUCTION_REHEARSAL_ROOT="$ROOT"
export LIVE_PRODUCTION_TEST_SERVER="$ROOT/target/release/live-production" LIVE_PRODUCTION_TEST_SUPERVISOR="$ROOT/target/release/live-production-supervisor" LIVE_PRODUCTION_TEST_WEB_DIST="$ROOT/web/dist"
export LIVE_PRODUCTION_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR" FFMPEG=/usr/bin/ffmpeg FFPROBE=/usr/bin/ffprobe
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1 LIVE_PRODUCTION_RECOVERY_ROOT="$ROOT"
export LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR" LIVE_PRODUCTION_RECOVERY_SERVER="$LIVE_PRODUCTION_TEST_SERVER" LIVE_PRODUCTION_RECOVERY_WEB_DIST="$ROOT/web/dist"
gate target-drivers "$SUP" --timeout-ms 600000 --grace-ms 10000 -- node --test --test-concurrency=1 scripts/target/*.test.mjs scripts/target/harness/*.test.mjs
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
GIT_MASTER=1 git status --porcelain > var/completion-06/source-status.txt
ps -eo pid,ppid,stat,comm > var/completion-06/processes-after.txt
sha256sum var/obs-plugin-stage/bin/64bit/obs-live-production.so target/release/live-production target/release/live-production-supervisor .tools/ubuntu/lib/gstreamer-1.0/libgstrswebrtc.so > var/completion-06/binary-hashes.txt
printf 'round_six_finished=%s failed=%s\n' "$(date -u +%FT%TZ)" "$failed"
exit "$failed"
