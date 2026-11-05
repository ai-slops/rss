#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/completion/runtime
chmod 700 var/completion/runtime
source scripts/ubuntu-env.sh
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/completion/runtime"
unset DISPLAY
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git merge-base --is-ancestor 2a7fdf6b741057e46aae18804dc5801804da48fe HEAD
GIT_MASTER=1 git merge-base --is-ancestor 9a07ef4beda93505a4c0d50485bc3c5f9df5df62 HEAD
{
  date -u +%FT%TZ
  GIT_MASTER=1 git rev-parse HEAD
  GIT_MASTER=1 git status --porcelain
  cat /proc/sys/kernel/random/boot_id
  cat /etc/os-release
  uname -a
  rustc --version; cargo --version; node --version; npm --version
  pkg-config --modversion gstreamer-1.0 libobs Qt6Widgets
  obs --version
  ffmpeg -version
  gst-inspect-1.0 rswebrtc
} > var/completion/inventory.txt
: > var/completion/results.tsv
failed=0
gate() {
  local name=$1 result=0 start
  shift
  start=$(date -u +%FT%TZ)
  "$@" > "var/completion/$name.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/completion/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 18 "var/completion/$name.log"; fi
}
gate debug-build cargo build --locked --workspace
gate release-build cargo build --locked --release --workspace
gate npm-ci npm --prefix web ci
gate web-build npm --prefix web run build
gate cmake-configure cmake -S integrations/obs-live-production -B var/obs-plugin-build -DCMAKE_BUILD_TYPE=RelWithDebInfo
gate cmake-build cmake --build var/obs-plugin-build --parallel 2
gate ctest env QT_QPA_PLATFORM=offscreen timeout 180 ctest --test-dir var/obs-plugin-build --output-on-failure
gate cmake-install cmake --install var/obs-plugin-build --prefix "$ROOT/var/obs-plugin-stage"
SUP="$ROOT/target/debug/live-production-supervisor"
test -x "$SUP"
gate identity "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test intervideo_provenance_identity -- --nocapture
gate provenance env LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR="$ROOT/var/completion/provenance" GST_DEBUG=referencetimestampmeta:7,intervideosrc:7 "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production --test editor_clean_provenance clean_recording_reconnect_provenance_survives_catalog_reload -- --nocapture
gate lifecycle env LIVE_PRODUCTION_LIFECYCLE_EXPECT_ZERO=1 bash scripts/lifecycle-acceptance.sh
gate media-network bash scripts/media-network-gates.sh "$ROOT/var/completion/media-network"
gate source-recovery-model "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production --lib source_recovery
gate source-recovery-http "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production --test http_source_recovery
gate obs-rehearsal scripts/target/rehearsal.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/obs-completion-01" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --obs-binary /usr/bin/obs --deadline-seconds 120
gate obs-recovery scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/recovery-completion-01" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
export LIVE_PRODUCTION_REHEARSAL_DRIVER=1 LIVE_PRODUCTION_REHEARSAL_ROOT="$ROOT"
export LIVE_PRODUCTION_TEST_SERVER="$ROOT/target/release/live-production"
export LIVE_PRODUCTION_TEST_SUPERVISOR="$ROOT/target/release/live-production-supervisor"
export LIVE_PRODUCTION_TEST_WEB_DIST="$ROOT/web/dist"
export LIVE_PRODUCTION_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR" FFMPEG=/usr/bin/ffmpeg FFPROBE=/usr/bin/ffprobe
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1 LIVE_PRODUCTION_RECOVERY_ROOT="$ROOT"
export LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR" LIVE_PRODUCTION_RECOVERY_SERVER="$LIVE_PRODUCTION_TEST_SERVER" LIVE_PRODUCTION_RECOVERY_WEB_DIST="$ROOT/web/dist"
gate target-drivers "$SUP" --timeout-ms 600000 --grace-ms 10000 -- node --test --test-concurrency=1 scripts/target/*.test.mjs scripts/target/harness/*.test.mjs
gate web-tests npm --prefix web test
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
printf 'finished_at=%s failed=%s\n' "$(date -u +%FT%TZ)" "$failed"
exit "$failed"
