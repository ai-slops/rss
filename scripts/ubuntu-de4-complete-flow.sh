#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
RUN=${LIVE_NATIVE_RUN_NAME:-de4-native-07}
mkdir -p "var/$RUN/runtime"
source scripts/ubuntu-env.sh
export CARGO_TARGET_DIR="$HOME/review88/target" LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/$RUN/runtime"
unset DISPLAY LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_OMIT LIVE_PRODUCTION_OBS_METER_DIAGNOSTIC
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git diff --exit-code de4b80058871f4ec5f138ddb25cb68d05c5bc718 HEAD -- server/src media/src integrations
GIT_MASTER=1 git rev-parse HEAD > "var/$RUN/source-head.txt"
result=0
scripts/target/recovery-native.sh --plugin-dir "$ROOT/var/de4-plugin-stage" --evidence-dir "$ROOT/var/$RUN/recovery-full" --server-binary "$ROOT/target/release/live-production" --supervisor-binary "$ROOT/target/release/live-production-supervisor" --web-dist "$ROOT/web/dist" --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300 > "var/$RUN/recovery-full.log" 2>&1 || result=$?
printf 'recovery-full\t%s\t%s\n' "$result" "$(date -u +%FT%TZ)" > "var/$RUN/results.tsv"
cargo fmt --all -- --check > "var/$RUN/fmt.log" 2>&1
cargo clippy --locked --workspace --all-targets -- -D warnings > "var/$RUN/clippy.log" 2>&1
GIT_MASTER=1 git status --porcelain > "var/$RUN/source-status.txt"
sha256sum var/de4-plugin-stage/bin/64bit/obs-live-production.so target/release/live-production target/release/live-production-supervisor > "var/$RUN/binary-hashes.txt"
exit "$result"
