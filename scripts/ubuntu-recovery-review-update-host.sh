#!/usr/bin/env bash
set -euo pipefail
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/recovery-review-update-20261106.bundle" live-ubuntu:recovery-review-update.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'set -euo pipefail; umask 077; cd review88; test -z "$(GIT_MASTER=1 git status --porcelain)"; GIT_MASTER=1 git fetch ../recovery-review-update.bundle HEAD; GIT_MASTER=1 git switch --detach FETCH_HEAD; bash scripts/ubuntu-recovery-review-second.sh' || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'umask 077; cd review88; tar -czf /home/validator/recovery-review-evidence-02-20261106.tar.gz var/recovery-review var/recovery-review-02; sha256sum /home/validator/recovery-review-evidence-02-20261106.tar.gz'
scp -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu:recovery-review-evidence-02-20261106.tar.gz "$LIVE_UBUNTU_STATE/recovery-review-evidence-02-20261106.tar.gz"
exit "$result"
