#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
if [ "${1:-}" = --guest-workload ]; then
  scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/source.bundle" live-ubuntu:source.bundle
  result=0
  ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu \
    'git clone source.bundle source && cd source && bash scripts/ubuntu-native-check.sh' || result=$?
  ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu \
    'cd source && tar -czf - var/ubuntu-native' > "$LIVE_UBUNTU_STATE/receipts.tar.gz"
  printf 'native_workload_exit=%s\n' "$result" | tee "$LIVE_UBUNTU_STATE/workload-result.txt"
  exit "$result"
fi
if [ "${1:-}" != run ]; then
  printf '%s\n' 'Usage: bash scripts/ubuntu-verify.sh run' 'Runs only committed, clean source in one owned Ubuntu guest; private receipts remain in var/ubuntu-validation.'
  exit 2
fi
test -z "$(GIT_MASTER=1 git status --porcelain)" || { printf '%s\n' 'Commit verification tooling before creating the clean-source bundle.' >&2; exit 1; }
umask 077
mkdir -p var/ubuntu-validation
GIT_MASTER=1 git bundle create var/ubuntu-validation/source.bundle HEAD
exec timeout --signal=TERM --kill-after=60 5400 \
  bash scripts/ubuntu-vm.sh run bash scripts/ubuntu-verify.sh --guest-workload
