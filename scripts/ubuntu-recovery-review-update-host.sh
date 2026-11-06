#!/usr/bin/env bash
set -euo pipefail
MODE=${1:-second}
case "$MODE" in second) SUFFIX=02 ;; engine) SUFFIX=03 ;; caps) SUFFIX=04 ;; capture) SUFFIX=05 ;; format) SUFFIX=06 ;; *) exit 2 ;; esac
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/recovery-review-update-20261106.bundle" live-ubuntu:recovery-review-update.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu "set -euo pipefail; umask 077; cd review88; test -z \"\$(GIT_MASTER=1 git status --porcelain)\"; GIT_MASTER=1 git fetch ../recovery-review-update.bundle HEAD; GIT_MASTER=1 git switch --detach FETCH_HEAD; bash scripts/ubuntu-recovery-review-$MODE.sh" || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu "umask 077; cd review88; tar -czf /home/validator/recovery-review-evidence-$SUFFIX-20261106.tar.gz var/recovery-review*; sha256sum /home/validator/recovery-review-evidence-$SUFFIX-20261106.tar.gz"
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "live-ubuntu:recovery-review-evidence-$SUFFIX-20261106.tar.gz" "$LIVE_UBUNTU_STATE/recovery-review-evidence-$SUFFIX-20261106.tar.gz"
exit "$result"
