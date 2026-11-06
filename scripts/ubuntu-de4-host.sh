#!/usr/bin/env bash
set -euo pipefail
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/de4-native-integration.bundle" live-ubuntu:de4-native-integration.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'set -euo pipefail; umask 077; test ! -e reviewde4; GIT_MASTER=1 git -C review88 fetch ../de4-native-integration.bundle HEAD; GIT_MASTER=1 git -C review88 worktree add --detach ../reviewde4 FETCH_HEAD; cd reviewde4; ln -s ../review88/.tools .tools; ln -s ../review88/target target; ln -s ../../review88/web/node_modules web/node_modules; bash scripts/ubuntu-de4-guest.sh' || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'umask 077; cd reviewde4; tar -czf /home/validator/de4-native-evidence-01.tar.gz var/de4-native var/de4-plugin-build/Testing; sha256sum /home/validator/de4-native-evidence-01.tar.gz'
scp -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu:de4-native-evidence-01.tar.gz "$LIVE_UBUNTU_STATE/de4-native-evidence-01.tar.gz"
exit "$result"
