#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
mkdir -p var/completion-08
source scripts/ubuntu-env.sh
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/completion/runtime"
unset DISPLAY
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git rev-parse HEAD > var/completion-08/source-head.txt
GIT_MASTER=1 git merge-base --is-ancestor 2a7fdf6b741057e46aae18804dc5801804da48fe HEAD
GIT_MASTER=1 git merge-base --is-ancestor 9a07ef4beda93505a4c0d50485bc3c5f9df5df62 HEAD
GIT_MASTER=1 git diff --exit-code 9a07ef4beda93505a4c0d50485bc3c5f9df5df62 HEAD -- server/src media/src integrations
SUP="$ROOT/target/debug/live-production-supervisor"
: > var/completion-08/results.tsv
failed=0; LAST_RESULT=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/completion-08/$name.log" 2>&1 || result=$?
  LAST_RESULT=$result
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/completion-08/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 18 "var/completion-08/$name.log"; fi
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
gate obs-rehearsal scripts/target/rehearsal.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/obs-completion-07" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --obs-binary /usr/bin/obs --deadline-seconds 120
gate recovery-full recovery recovery-completion-20
gate recovery-fences env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=fences bash -c 'recovery recovery-completion-21'
gate recovery-budget env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=budget bash -c 'recovery recovery-completion-22'
gate recovery-controls env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=controls bash -c 'recovery recovery-completion-23'
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
{ date -u +%FT%TZ; cat /proc/sys/kernel/random/boot_id; uname -a; rustc --version; cargo --version; node --version; npm --version; pkg-config --modversion gstreamer-1.0 libobs Qt6Widgets; obs --version; } > var/completion-08/final-versions.txt
dpkg-query -W > var/completion-08/packages.tsv
GIT_MASTER=1 git status --porcelain > var/completion-08/source-status.txt
ps -eo pid,ppid,stat,comm > var/completion-08/processes-after.txt
sha256sum var/obs-plugin-stage/bin/64bit/obs-live-production.so target/release/live-production target/release/live-production-supervisor .tools/ubuntu/lib/gstreamer-1.0/libgstrswebrtc.so > var/completion-08/binary-hashes.txt
printf 'round_eight_finished=%s failed=%s\n' "$(date -u +%FT%TZ)" "$failed"
exit "$failed"
