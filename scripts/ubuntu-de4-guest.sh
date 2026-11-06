#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
mkdir -p var/de4-native/runtime
source scripts/ubuntu-env.sh
export CARGO_TARGET_DIR="$HOME/review88/target"
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/de4-native/runtime"
unset DISPLAY LIVE_PRODUCTION_OBS_METER_DIAGNOSTIC LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_OMIT
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git merge-base --is-ancestor de4b80058871f4ec5f138ddb25cb68d05c5bc718 HEAD
GIT_MASTER=1 git merge-base --is-ancestor e6afd84b26c428696dc922b5c489b6ddcc1136f5 HEAD
GIT_MASTER=1 git diff --exit-code de4b80058871f4ec5f138ddb25cb68d05c5bc718 HEAD -- server/src media/src integrations
GIT_MASTER=1 git rev-parse HEAD > var/de4-native/source-head.txt
{ date -u +%FT%TZ; uname -a; rustc --version; cargo --version; node --version; pkg-config --modversion gstreamer-1.0 libobs Qt6Widgets; obs --version; } > var/de4-native/inventory.txt
: > var/de4-native/results.tsv
failed=0; LAST_RESULT=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/de4-native/$name.log" 2>&1 || result=$?
  LAST_RESULT=$result
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/de4-native/results.tsv
  if [ "$result" != 0 ]; then failed=1; fi
}
gate debug-build cargo build --locked --workspace
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
gate release-build cargo build --locked --release --workspace
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
gate web-build npm --prefix web run build
gate cmake-configure cmake -S integrations/obs-live-production -B var/de4-plugin-build -DCMAKE_BUILD_TYPE=RelWithDebInfo
gate cmake-build cmake --build var/de4-plugin-build --parallel 2
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
gate ctest env QT_QPA_PLATFORM=offscreen timeout 180 ctest --test-dir var/de4-plugin-build --output-on-failure
gate cmake-install cmake --install var/de4-plugin-build --prefix "$ROOT/var/de4-plugin-stage"
if [ "$LAST_RESULT" != 0 ]; then exit 1; fi
SUP="$ROOT/target/debug/live-production-supervisor"
export LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/de4-native/media"
gate obs-rehearsal scripts/target/rehearsal.sh --plugin-dir "$ROOT/var/de4-plugin-stage" --evidence-dir "$ROOT/var/de4-native/obs-rehearsal" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --obs-binary /usr/bin/obs --deadline-seconds 120
recovery() {
  scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/de4-plugin-stage" --evidence-dir "$ROOT/var/de4-native/$1" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
}
export -f recovery
gate recovery-full recovery recovery-full
gate recovery-fences env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=fences bash -c 'recovery recovery-fences'
gate recovery-budget env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=budget bash -c 'recovery recovery-budget'
gate recovery-controls env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=controls bash -c 'recovery recovery-controls'
gate provenance env LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR="$ROOT/var/de4-native/provenance" "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production --test editor_clean_provenance --test editor_clean_metadata_free -- --nocapture
gate program-recording "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test runtime_recording_contract --test source_capture_recording_contract --test source_capture_lag_contract -- --nocapture
gate drift-rss "$SUP" --timeout-ms 300000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test srt_drift_endurance -- --nocapture
gate output-lifecycle "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production-media --lib output_runtime -- --nocapture
gate recovery-unit "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production --lib source_recovery -- --nocapture
gate recovery-http "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production --test http_source_recovery -- --nocapture
gate engine-run-live "$SUP" --timeout-ms 180000 --grace-ms 10000 -- node scripts/target/recovery-engine-run.driver.mjs "$ROOT/var/de4-native/engine-run-live"
export LIVE_PRODUCTION_REHEARSAL_DRIVER=1 LIVE_PRODUCTION_REHEARSAL_ROOT="$ROOT"
export LIVE_PRODUCTION_TEST_SERVER="$ROOT/target/release/live-production" LIVE_PRODUCTION_TEST_SUPERVISOR="$ROOT/target/release/live-production-supervisor" LIVE_PRODUCTION_TEST_WEB_DIST="$ROOT/web/dist"
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1 LIVE_PRODUCTION_RECOVERY_ROOT="$ROOT"
export LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR" LIVE_PRODUCTION_RECOVERY_SERVER="$LIVE_PRODUCTION_TEST_SERVER" LIVE_PRODUCTION_RECOVERY_WEB_DIST="$ROOT/web/dist"
export FFMPEG=/usr/bin/ffmpeg FFPROBE=/usr/bin/ffprobe
gate target-tests "$SUP" --timeout-ms 600000 --grace-ms 10000 -- node --test --test-concurrency=1 scripts/target/*.test.mjs scripts/target/harness/*.test.mjs
gate web-tests npm --prefix web test
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
GIT_MASTER=1 git status --porcelain > var/de4-native/source-status.txt
sha256sum var/de4-plugin-stage/bin/64bit/obs-live-production.so target/release/live-production target/release/live-production-supervisor > var/de4-native/binary-hashes.txt
ps -eo pid,ppid,stat,comm > var/de4-native/processes-after.txt
exit "$failed"
