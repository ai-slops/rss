#!/usr/bin/env bash
set -euo pipefail
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/eos-matrix-update.bundle" live-ubuntu:eos-matrix-update.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'set -euo pipefail; umask 077; cd reviewde4; test -z "$(GIT_MASTER=1 git status --porcelain)"; GIT_MASTER=1 git fetch ../eos-matrix-update.bundle HEAD; GIT_MASTER=1 git switch --detach FETCH_HEAD; bash scripts/ubuntu-eos-matrix-guest.sh' || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'umask 077; cd reviewde4; tar -czf /home/validator/eos-native-evidence-01-20261106.tar.gz var/de4-eos-matrix; sha256sum /home/validator/eos-native-evidence-01-20261106.tar.gz'
scp -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu:eos-native-evidence-01-20261106.tar.gz "$LIVE_UBUNTU_STATE/eos-native-evidence-01-20261106.tar.gz"
exit "$result"
