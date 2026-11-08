#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
mkdir -p var/completion-03
source scripts/ubuntu-env.sh
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/completion/runtime"
unset DISPLAY
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git rev-parse HEAD > var/completion-03/source-head.txt
SUP="$ROOT/target/debug/live-production-supervisor"
: > var/completion-03/results.tsv
failed=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/completion-03/$name.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/completion-03/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 16 "var/completion-03/$name.log"; fi
}
recovery() {
  scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/$1" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
}
export -f recovery
gate media-inspection "$SUP" --timeout-ms 180000 --grace-ms 5000 -- node scripts/target/inspect-native-media.mjs "$SUP" "$ROOT/var/completion-03/media-inspection.json" "$ROOT/var/obs-completion-01/private/state/recordings" "$ROOT/var/completion/provenance" "$ROOT/var/recovery-completion-06/private/state/recordings"
gate obs-rehearsal scripts/target/rehearsal.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/obs-completion-03" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --obs-binary /usr/bin/obs --deadline-seconds 120
gate recovery-full recovery recovery-completion-07
gate recovery-fences env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=fences bash -c 'recovery recovery-completion-08'
gate recovery-budget env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=budget bash -c 'recovery recovery-completion-09'
gate recovery-controls env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=controls bash -c 'recovery recovery-completion-10'
export LIVE_PRODUCTION_REHEARSAL_DRIVER=1 LIVE_PRODUCTION_REHEARSAL_ROOT="$ROOT"
export LIVE_PRODUCTION_TEST_SERVER="$ROOT/target/release/live-production" LIVE_PRODUCTION_TEST_SUPERVISOR="$ROOT/target/release/live-production-supervisor" LIVE_PRODUCTION_TEST_WEB_DIST="$ROOT/web/dist"
export LIVE_PRODUCTION_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR" FFMPEG=/usr/bin/ffmpeg FFPROBE=/usr/bin/ffprobe
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1 LIVE_PRODUCTION_RECOVERY_ROOT="$ROOT"
export LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR" LIVE_PRODUCTION_RECOVERY_SERVER="$LIVE_PRODUCTION_TEST_SERVER" LIVE_PRODUCTION_RECOVERY_WEB_DIST="$ROOT/web/dist"
gate target-drivers "$SUP" --timeout-ms 600000 --grace-ms 10000 -- node --test --test-concurrency=1 scripts/target/*.test.mjs scripts/target/harness/*.test.mjs
sha256sum var/obs-plugin-stage/bin/64bit/obs-live-production.so target/release/live-production target/release/live-production-supervisor .tools/ubuntu/lib/gstreamer-1.0/libgstrswebrtc.so > var/completion-03/binary-hashes.txt
GIT_MASTER=1 git status --porcelain > var/completion-03/source-status.txt
ps -eo pid,ppid,stat,comm > var/completion-03/processes-after.txt
printf 'round_three_finished=%s\n' "$(date -u +%FT%TZ)"
exit "$failed"
