#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/recovery-review-03
source scripts/ubuntu-env.sh
GIT_MASTER=1 git rev-parse HEAD > var/recovery-review-03/source-head.txt
result=0
target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- node scripts/target/recovery-engine-run.driver.mjs "$ROOT/var/recovery-review-03/engine-run-live" > var/recovery-review-03/engine-run-live.log 2>&1 || result=$?
printf 'engine-run-live\t%s\t%s\n' "$result" "$(date -u +%FT%TZ)" | tee var/recovery-review-03/results.tsv
node --test scripts/target/harness/program-content-fence.test.mjs > var/recovery-review-03/program-content-unit.log 2>&1
GIT_MASTER=1 git status --porcelain > var/recovery-review-03/source-status.txt
ps -eo pid,ppid,stat,comm > var/recovery-review-03/processes-after.txt
exit "$result"
