#!/usr/bin/env bash
set -euo pipefail
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/de4-native-integration.bundle" live-ubuntu:de4-native-integration.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'set -euo pipefail; umask 077; cd reviewde4; test "$(readlink target)" = ../review88/target; unlink target; mkdir target; ln -s ../../review88/target/debug target/debug; ln -s ../../review88/target/release target/release; test -z "$(GIT_MASTER=1 git status --porcelain)"; GIT_MASTER=1 git fetch ../de4-native-integration.bundle HEAD; GIT_MASTER=1 git switch --detach FETCH_HEAD; bash scripts/ubuntu-de4-guest.sh' || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'umask 077; cd reviewde4; tar -czf /home/validator/de4-native-evidence-02.tar.gz var/de4-native; sha256sum /home/validator/de4-native-evidence-02.tar.gz'
scp -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu:de4-native-evidence-02.tar.gz "$LIVE_UBUNTU_STATE/de4-native-evidence-02.tar.gz"
exit "$result"
