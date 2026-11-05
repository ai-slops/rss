#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/native-completion-update-02.bundle" live-ubuntu:completion-update.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'set -euo pipefail; umask 077; cd completion; test -z "$(GIT_MASTER=1 git status --porcelain)"; GIT_MASTER=1 git fetch ../completion-update.bundle HEAD; GIT_MASTER=1 git switch --detach FETCH_HEAD; bash scripts/ubuntu-completion-round2.sh' || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'umask 077; cd completion; tar -czf /home/validator/native-completion-evidence-02.tar.gz var; sha256sum /home/validator/native-completion-evidence-02.tar.gz'
scp -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu:native-completion-evidence-02.tar.gz "$LIVE_UBUNTU_STATE/native-completion-evidence-02.tar.gz"
exit "$result"
