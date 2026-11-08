#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
mkdir -p var/completion-05
source scripts/ubuntu-env.sh
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/completion/runtime"
unset DISPLAY
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git rev-parse HEAD > var/completion-05/source-head.txt
SUP="$ROOT/target/debug/live-production-supervisor"
: > var/completion-05/results.tsv
failed=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/completion-05/$name.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/completion-05/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 18 "var/completion-05/$name.log"; fi
}
gate cmake-build cmake --build var/obs-plugin-build --parallel 2
gate ctest env QT_QPA_PLATFORM=offscreen timeout 180 ctest --test-dir var/obs-plugin-build --output-on-failure
gate cmake-install cmake --install var/obs-plugin-build --prefix "$ROOT/var/obs-plugin-stage"
gate obs-rehearsal scripts/target/rehearsal.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/obs-completion-04" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --obs-binary /usr/bin/obs --deadline-seconds 120
gate recovery-full scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/recovery-completion-11" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
gate program-recording env LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/completion-05/media" "$SUP" --timeout-ms 180000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test runtime_recording_contract -- --nocapture
gate drift-rss env LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR="$ROOT/var/completion-05/media" "$SUP" --timeout-ms 300000 --grace-ms 10000 -- cargo test --locked -p live-production-media --test srt_drift_endurance -- --nocapture
for file in var/completion-05/media/*/*/*.mkv; do
  [ -f "$file" ] || continue
  relative=${file#var/completion-05/media/}
  base="var/completion-05/analysis/$relative"
  mkdir -p "$(dirname "$base")"
  sha256sum "$file" >> var/completion-05/media-sha256.txt
  result=0; ffprobe -v error -show_streams -show_format -of json "$file" > "$base.ffprobe.json" 2> "$base.ffprobe.stderr" || result=$?
  printf '%s\tffprobe\t%s\n' "$relative" "$result" >> var/completion-05/media-results.tsv
  if [ "$result" != 0 ]; then failed=1; fi
  result=0; "$SUP" --timeout-ms 30000 --grace-ms 5000 -- ffmpeg -v error -xerror -i "$file" -f null - > "$base.decode.log" 2>&1 || result=$?
  printf '%s\tstrict-decode\t%s\n' "$relative" "$result" >> var/completion-05/media-results.tsv
  if [ "$result" != 0 ]; then failed=1; fi
done
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
sha256sum var/obs-plugin-stage/bin/64bit/obs-live-production.so target/release/live-production target/release/live-production-supervisor .tools/ubuntu/lib/gstreamer-1.0/libgstrswebrtc.so > var/completion-05/binary-hashes.txt
GIT_MASTER=1 git status --porcelain > var/completion-05/source-status.txt
ps -eo pid,ppid,stat,comm > var/completion-05/processes-after.txt
printf 'round_five_finished=%s\n' "$(date -u +%FT%TZ)"
exit "$failed"
