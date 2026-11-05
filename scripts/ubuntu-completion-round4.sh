#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/completion-04
source scripts/ubuntu-env.sh
export LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR="$ROOT/var/completion/runtime"
unset DISPLAY
test -z "$(GIT_MASTER=1 git status --porcelain)"
GIT_MASTER=1 git rev-parse HEAD > var/completion-04/source-head.txt
: > var/completion-04/results.tsv
SUP="$ROOT/target/debug/live-production-supervisor"
gate() {
  local name=$1 result=0 start
  shift; start=$(date -u +%FT%TZ)
  "$@" > "var/completion-04/$name.log" 2>&1 || result=$?
  printf '%s\t%s\t%s\t%s\n' "$name" "$result" "$start" "$(date -u +%FT%TZ)" | tee -a var/completion-04/results.tsv
  if [ "$result" != 0 ]; then tail -n 18 "var/completion-04/$name.log"; fi
}
dpkg-query -W > var/completion-04/packages-before.tsv
gate chromium-install timeout 300 node web/node_modules/playwright/cli.js install --with-deps chromium
gate browser-native bash scripts/browser-acceptance.sh "$ROOT/var/browser-completion-01"
dpkg-query -W > var/completion-04/packages-after.tsv
gate rust-workspace env LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR="$ROOT/var/completion-04/provenance" "$SUP" --timeout-ms 900000 --grace-ms 10000 -- cargo test --locked --no-fail-fast --workspace -- --test-threads=1 --nocapture
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy --locked --workspace --all-targets -- -D warnings
GIT_MASTER=1 git merge-base --is-ancestor 2a7fdf6b741057e46aae18804dc5801804da48fe HEAD
GIT_MASTER=1 git merge-base --is-ancestor 9a07ef4beda93505a4c0d50485bc3c5f9df5df62 HEAD
GIT_MASTER=1 git diff --exit-code 9a07ef4beda93505a4c0d50485bc3c5f9df5df62 HEAD -- server/src media/src integrations
GIT_MASTER=1 git status --porcelain > var/completion-04/source-status.txt
{ date -u +%FT%TZ; cat /proc/sys/kernel/random/boot_id; rustc --version; cargo --version; node --version; npm --version; pkg-config --modversion gstreamer-1.0 libobs Qt6Widgets; obs --version; } > var/completion-04/final-versions.txt
ps -eo pid,ppid,stat,comm > var/completion-04/processes-after.txt
printf 'round_four_finished=%s\n' "$(date -u +%FT%TZ)"
