#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export ROOT
cd "$ROOT"
umask 077
mkdir -p var/completion-02
source scripts/ubuntu-env.sh
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/completion/runtime"
unset DISPLAY
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git rev-parse HEAD > var/completion-02/source-head.txt
SUP="$ROOT/target/debug/live-production-supervisor"
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1 LIVE_PRODUCTION_RECOVERY_ROOT="$ROOT"
export LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$ROOT/target/release/live-production-supervisor" LIVE_PRODUCTION_RECOVERY_SERVER="$ROOT/target/release/live-production" LIVE_PRODUCTION_RECOVERY_WEB_DIST="$ROOT/web/dist"
: > var/completion-02/results.tsv
failed=0
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/completion-02/$name.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/completion-02/results.tsv
  if [ "$result" != 0 ]; then failed=1; tail -n 16 "var/completion-02/$name.log"; fi
}
recovery() {
  scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/$1" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
}
export -f recovery
gate media-inspection "$SUP" --timeout-ms 120000 --grace-ms 5000 -- node scripts/target/inspect-native-media.mjs "$SUP" "$ROOT/var/completion-02/media-inspection.json" "$ROOT/var/obs-completion-01/private/state/recordings" "$ROOT/var/completion/provenance"
gate recovery-local "$SUP" --timeout-ms 180000 --grace-ms 10000 -- node --test scripts/target/recovery-local.driver.test.mjs
gate obs-rehearsal scripts/target/rehearsal.sh --plugin-dir "$ROOT/var/obs-plugin-stage" --evidence-dir "$ROOT/var/obs-completion-02" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --obs-binary /usr/bin/obs --deadline-seconds 120
gate recovery-full recovery recovery-completion-02
gate recovery-after-freeze env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_OMIT=freezePublisher bash -c 'recovery recovery-completion-03'
gate recovery-later-cases env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_OMIT=freezePublisher,obsProcessSigstop bash -c 'recovery recovery-completion-04'
gate recovery-fences env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=fences bash -c 'recovery recovery-completion-05'
gate recovery-budget env LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=budget bash -c 'recovery recovery-completion-06'
gate rust-workspace env LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR="$ROOT/var/completion-02/full-suite-provenance" "$SUP" --timeout-ms 900000 --grace-ms 10000 -- cargo test --locked --no-fail-fast --workspace -- --test-threads=1
gate harness-unit "$SUP" --timeout-ms 180000 --grace-ms 10000 -- node --test --test-concurrency=1 scripts/target/harness/*.test.mjs
printf 'round_two_finished=%s\n' "$(date -u +%FT%TZ)"
exit "$failed"
