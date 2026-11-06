#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
mkdir -p var/de4-native-03/runtime
source scripts/ubuntu-env.sh
export CARGO_TARGET_DIR="$HOME/review88/target" LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/de4-native-03/runtime"
unset DISPLAY LIVE_PRODUCTION_OBS_METER_DIAGNOSTIC LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_OMIT
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git diff --exit-code de4b80058871f4ec5f138ddb25cb68d05c5bc718 HEAD -- server/src media/src integrations
GIT_MASTER=1 git rev-parse HEAD > var/de4-native-03/source-head.txt
sha256sum var/de4-plugin-stage/bin/64bit/obs-live-production.so target/release/live-production target/release/live-production-supervisor > var/de4-native-03/binary-hashes.txt
: > var/de4-native-03/results.tsv
failed=0; LAST_RESULT=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/de4-native-03/$name.log" 2>&1 || result=$?
  LAST_RESULT=$result
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/de4-native-03/results.tsv
  if [ "$result" != 0 ]; then failed=1; fi
}
gate npm-ci npm --prefix web ci
gate web-build npm --prefix web run build
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
recovery() {
  scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/de4-plugin-stage" --evidence-dir "$ROOT/var/de4-native-03/$1" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
}
export -f recovery
gate recovery-full recovery recovery-full
gate recovery-fences env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=fences bash -c 'recovery recovery-fences'
gate recovery-budget env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=budget bash -c 'recovery recovery-budget'
gate recovery-controls env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=controls bash -c 'recovery recovery-controls'
SUP="$ROOT/target/debug/live-production-supervisor"
gate drift-rss env LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/de4-native-03/media" GST_DEBUG=splitmuxsink:5,matroskamux:4 "$SUP" --timeout-ms 300000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test srt_drift_endurance -- --nocapture
gate web-tests npm --prefix web test
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
GIT_MASTER=1 git status --porcelain > var/de4-native-03/source-status.txt
ps -eo pid,ppid,stat,comm > var/de4-native-03/processes-after.txt
exit "$failed"
