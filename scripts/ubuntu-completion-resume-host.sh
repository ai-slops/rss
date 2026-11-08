#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
ROUND=${1:-2}
case "$ROUND" in 2|3|4|5|6|7|8) ;; *) exit 2 ;; esac
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/native-completion-update-0$ROUND.bundle" live-ubuntu:completion-update.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu "set -euo pipefail; umask 077; cd completion; test -z \"\$(GIT_MASTER=1 git status --porcelain)\"; GIT_MASTER=1 git fetch ../completion-update.bundle HEAD; GIT_MASTER=1 git switch --detach FETCH_HEAD; bash scripts/ubuntu-completion-round$ROUND.sh" || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu "umask 077; cd completion; tar -czf /home/validator/native-completion-evidence-0$ROUND.tar.gz var; sha256sum /home/validator/native-completion-evidence-0$ROUND.tar.gz"
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "live-ubuntu:native-completion-evidence-0$ROUND.tar.gz" "$LIVE_UBUNTU_STATE/native-completion-evidence-0$ROUND.tar.gz"
exit "$result"
