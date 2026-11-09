#!/usr/bin/env bash
# Run in the dedicated guest after ubuntu-dependencies.sh.
set -euo pipefail
cd "$(dirname -- "$0")/.."
source scripts/ubuntu-env.sh
umask 077
mkdir -p var
RUN=$(mktemp -d "$PWD/var/obs-check.XXXXXX")
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$RUN/runtime"
mkdir -m 700 "$XDG_RUNTIME_DIR"
unset DISPLAY QT_QPA_PLATFORM LIVE_OBS_SDK_ROOT LIVE_LOCAL_OBS_OPENGL
unset LIVE_PRODUCTION_OBS_FIXTURE_FILE LIVE_PRODUCTION_OBS_RECOVERY_FIXTURE_FILE
unset LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_OMIT
failed=0
gate() {
  local name=$1 result=0
  shift
  printf 'START %s %s\n' "$name" "$(date -u +%FT%TZ)"
  "$@" > "$RUN/$name.log" 2>&1 || result=$?
  printf '%s\t%s\n' "$name" "$result" | tee -a "$RUN/results.tsv"
  if [ "$result" != 0 ]; then failed=1; tail -n 60 "$RUN/$name.log"; fi
  return "$result"
}
gate cargo-build cargo build --locked --workspace
gate web-install npm --prefix web ci --no-fund --no-audit
gate web-build npm --prefix web run build
gate plugin-configure cmake -S integrations/obs-live-production -B "$RUN/build" -DCMAKE_BUILD_TYPE=RelWithDebInfo
gate plugin-build cmake --build "$RUN/build" --parallel 2
gate plugin-tests env QT_QPA_PLATFORM=offscreen ctest --test-dir "$RUN/build" --output-on-failure || true
gate plugin-stage cmake --install "$RUN/build" --prefix "$RUN/package"
gate plugin-package bash scripts/package-obs-plugin.sh "$RUN/package" "$RUN/obs-live-production-linux-$(uname -m).tar.gz"
SUPERVISOR="$PWD/target/debug/live-production-supervisor"
SERVER="$PWD/target/debug/live-production"
gate rehearsal scripts/target/rehearsal.sh --plugin-dir "$RUN/package" --evidence-dir "$RUN/rehearsal" \
  --server-binary "$SERVER" --supervisor-binary "$SUPERVISOR" --obs-binary /usr/bin/obs --deadline-seconds 180 || true
gate recovery scripts/target/recovery-native.sh --plugin-dir "$RUN/package" --evidence-dir "$RUN/recovery" \
  --server-binary "$SERVER" --supervisor-binary "$SUPERVISOR" --web-dist "$PWD/web/dist" \
  --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300 || true
export LIVE_PRODUCTION_REHEARSAL_DRIVER=1 LIVE_PRODUCTION_REHEARSAL_ROOT="$PWD"
export LIVE_PRODUCTION_TEST_SERVER="$SERVER" LIVE_PRODUCTION_TEST_SUPERVISOR="$SUPERVISOR"
export LIVE_PRODUCTION_TEST_WEB_DIST="$PWD/web/dist" LIVE_PRODUCTION_SUPERVISOR="$SUPERVISOR"
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1
export LIVE_PRODUCTION_RECOVERY_ROOT="$PWD" LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$SUPERVISOR"
export LIVE_PRODUCTION_RECOVERY_SERVER="$SERVER" LIVE_PRODUCTION_RECOVERY_WEB_DIST="$PWD/web/dist"
export FFMPEG=/usr/bin/ffmpeg FFPROBE=/usr/bin/ffprobe
gate target-tests node --test --test-concurrency=1 scripts/target/harness/*.test.mjs scripts/target/*.test.mjs || true
gate software bash scripts/acceptance.sh --quick || true
printf 'RESULT failed=%s logs=%s\n' "$failed" "$RUN"
exit "$failed"
