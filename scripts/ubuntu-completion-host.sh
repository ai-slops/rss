#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
test -n "${LIVE_UBUNTU_SSH_CONFIG:-}"
test -n "${LIVE_UBUNTU_STATE:-}"
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/native-completion-20261105.bundle" live-ubuntu:completion.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'set -euo pipefail; umask 077; GIT_MASTER=1 git clone completion.bundle completion; cd completion; mkdir -p .tools/ubuntu/lib/gstreamer-1.0; cp -a ../integration/.tools/node .tools/node; cp ../integration/.tools/ubuntu/lib/gstreamer-1.0/libgstrswebrtc.so .tools/ubuntu/lib/gstreamer-1.0/; bash scripts/ubuntu-completion-guest.sh' || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'umask 077; cd completion; tar -czf /home/validator/native-completion-evidence-01.tar.gz var; sha256sum /home/validator/native-completion-evidence-01.tar.gz'
scp -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu:native-completion-evidence-01.tar.gz "$LIVE_UBUNTU_STATE/native-completion-evidence-01.tar.gz"
printf 'native_completion_workload_exit=%s\n' "$result"
exit "$result"
