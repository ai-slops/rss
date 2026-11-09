#!/usr/bin/env bash
# Own one isolated VM and test a snapshot of the current worktree.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
if [ "${1:-}" = --guest-workload ]; then
  scp -F "$LIVE_UBUNTU_SSH_CONFIG" "$LIVE_UBUNTU_STATE/source.tar.gz" live-ubuntu:source.tar.gz
  result=0
  ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu 'bash -s' <<'GUEST' || result=$?
set -euo pipefail
umask 077
mkdir source
tar xzf source.tar.gz -C source
cd source
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y curl ca-certificates git
curl --fail --proto '=https' --tlsv1.2 https://sh.rustup.rs -o /tmp/live-rustup-init.sh
sh /tmp/live-rustup-init.sh -y --profile minimal --default-toolchain 1.91.1 --component rustfmt,clippy
git init -q
git add .
git -c user.name=Validator -c user.email=validator@localhost commit -qm snapshot
bash scripts/ubuntu-dependencies.sh
bash scripts/ubuntu-obs-check.sh
GUEST
  ssh -F "$LIVE_UBUNTU_SSH_CONFIG" live-ubuntu \
    'cd source; tar -czf - var/obs-check.* var/ubuntu-dependencies' > "$LIVE_UBUNTU_STATE/results.tar.gz" || result=1
  printf 'native_workload_exit=%s\n' "$result" | tee "$LIVE_UBUNTU_STATE/workload-result.txt"
  exit "$result"
fi
if [ "${1:-}" != run ]; then
  printf '%s\n' 'Usage: bash scripts/ubuntu-verify.sh run' \
    'Requires QEMU/KVM, cloud-localds, OpenSSH, Node, curl, Git and tar.' \
    'Set LIVE_UBUNTU_STATE to a fresh directory (default: var/ubuntu-validation).'
  exit 2
fi
umask 077
export LIVE_UBUNTU_STATE=${LIVE_UBUNTU_STATE:-"$ROOT/var/ubuntu-validation"}
mkdir -p "$LIVE_UBUNTU_STATE"
LIVE_UBUNTU_STATE=$(realpath "$LIVE_UBUNTU_STATE")
test ! -e "$LIVE_UBUNTU_STATE/disk.qcow2" || { printf '%s\n' 'Choose a fresh LIVE_UBUNTU_STATE; an existing VM will not be overwritten.' >&2; exit 1; }
export LIVE_UBUNTU_IMAGE=${LIVE_UBUNTU_IMAGE:-"$LIVE_UBUNTU_STATE/base.img"}
if [ ! -f "$LIVE_UBUNTU_IMAGE" ]; then
  IMAGE_ORIGIN=https://cloud-images.ubuntu.com/noble/current
  curl --fail --location --proto '=https' --tlsv1.2 "$IMAGE_ORIGIN/SHA256SUMS" -o "$LIVE_UBUNTU_STATE/SHA256SUMS"
  LIVE_UBUNTU_IMAGE_SHA256=$(awk '$2 == "*noble-server-cloudimg-amd64.img" || $2 == "noble-server-cloudimg-amd64.img" {print $1}' "$LIVE_UBUNTU_STATE/SHA256SUMS")
  [[ $LIVE_UBUNTU_IMAGE_SHA256 =~ ^[a-f0-9]{64}$ ]]
  curl --fail --location --proto '=https' --tlsv1.2 "$IMAGE_ORIGIN/noble-server-cloudimg-amd64.img" -o "$LIVE_UBUNTU_IMAGE.partial"
  printf '%s  %s\n' "$LIVE_UBUNTU_IMAGE_SHA256" "$LIVE_UBUNTU_IMAGE.partial" | sha256sum -c -
  mv "$LIVE_UBUNTU_IMAGE.partial" "$LIVE_UBUNTU_IMAGE"
fi
export LIVE_UBUNTU_IMAGE_SHA256=${LIVE_UBUNTU_IMAGE_SHA256:?Supply the SHA-256 for an existing LIVE_UBUNTU_IMAGE}
LIVE_UBUNTU_IMAGE=$(realpath "$LIVE_UBUNTU_IMAGE")
tar -czf "$LIVE_UBUNTU_STATE/source.tar.gz" --null -T <(
  git ls-files -z --cached --others --exclude-standard | while IFS= read -r -d '' path; do
    if [ -f "$path" ]; then printf '%s\0' "$path"; fi
  done
)
sha256sum "$LIVE_UBUNTU_STATE/source.tar.gz" > "$LIVE_UBUNTU_STATE/source.sha256"
exec timeout --signal=TERM --kill-after=60 7200 \
  bash scripts/ubuntu-vm.sh run bash scripts/ubuntu-verify.sh --guest-workload
