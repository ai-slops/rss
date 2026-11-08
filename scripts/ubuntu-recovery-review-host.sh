#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/recovery-review-20261106.bundle" live-ubuntu:recovery-review.bundle
result=0
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'set -euo pipefail; umask 077; GIT_MASTER=1 git clone recovery-review.bundle review88; cd review88; mkdir -p .tools/ubuntu/lib/gstreamer-1.0 var; cp -a ../completion/.tools/node .tools/node; cp ../completion/.tools/ubuntu/lib/gstreamer-1.0/libgstrswebrtc.so .tools/ubuntu/lib/gstreamer-1.0/; cp -a ../completion/var/obs-plugin-stage var/obs-plugin-stage; cp -a ../completion/web/dist web/dist; bash scripts/ubuntu-recovery-review-guest.sh' || result=$?
ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'umask 077; cd review88; tar -czf /home/validator/recovery-review-evidence-20261106.tar.gz var/recovery-review; sha256sum /home/validator/recovery-review-evidence-20261106.tar.gz'
scp -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu:recovery-review-evidence-20261106.tar.gz "$LIVE_UBUNTU_STATE/recovery-review-evidence-20261106.tar.gz"
exit "$result"
