#!/usr/bin/env bash
# One foreground-owned disposable VM. The caller supplies a bounded host workload.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
STATE=${LIVE_UBUNTU_STATE:-"$ROOT/var/ubuntu-validation"}
MODE=${1:-}
RUN_ID=$(date -u +%Y%m%dT%H%M%SZ)
case ${1:-} in
  --help) printf '%s\n' 'Usage: bash scripts/ubuntu-vm.sh run|resume HOST-COMMAND [ARGS...]' 'Set LIVE_UBUNTU_IMAGE and LIVE_UBUNTU_IMAGE_SHA256. Set LIVE_UBUNTU_STATE when resuming.'; exit 0 ;;
  run|resume) shift; [ "$#" -gt 0 ] || { printf '%s\n' 'A workload command is required.' >&2; exit 2; } ;;
  *) printf '%s\n' 'Expected run, resume or --help.' >&2; exit 2 ;;
esac
BASE=${LIVE_UBUNTU_IMAGE:?Set LIVE_UBUNTU_IMAGE to an Ubuntu 24.04 cloud image}
DIGEST=${LIVE_UBUNTU_IMAGE_SHA256:?Set LIVE_UBUNTU_IMAGE_SHA256 to its verified SHA-256}
umask 077
mkdir -p "$STATE"
chmod 700 "$STATE"
exec 9>"$STATE/owner.lock"
flock -n 9 || { printf '%s\n' 'This VM already has an owner.' >&2; exit 1; }
printf '%s  %s\n' "$DIGEST" "$BASE" | sha256sum -c -
if [ "$MODE" = resume ]; then
  for owned in disk.qcow2 seed.img client host known_hosts ssh_config; do
    test -f "$STATE/$owned" || { printf 'Retained file missing: %s\n' "$owned" >&2; exit 1; }
  done
  test "$(qemu-img info --output=json "$STATE/disk.qcow2" | node -e 'let s="";process.stdin.on("data",v=>s+=v).on("end",()=>process.stdout.write(JSON.parse(s)["full-backing-filename"]));')" = "$BASE"
else
[ ! -e "$STATE/disk.qcow2" ] || { printf '%s\n' 'Refusing to overwrite an existing disposable disk.' >&2; exit 1; }
qemu-img info "$BASE"
ssh-keygen -q -t ed25519 -N '' -f "$STATE/client"
ssh-keygen -q -t ed25519 -N '' -f "$STATE/host"
{
  printf '#cloud-config\nhostname: live-ubuntu-validation\nusers:\n  - name: validator\n    shell: /bin/bash\n    sudo: ALL=(ALL) NOPASSWD:ALL\n    ssh_authorized_keys:\n      - '
  cut -d ' ' -f 1,2 "$STATE/client.pub"
  printf 'ssh_pwauth: false\ndisable_root: true\nssh_keys:\n  ed25519_private: |\n'
  sed 's/^/    /' "$STATE/host"
  printf '  ed25519_public: '
  cut -d ' ' -f 1,2 "$STATE/host.pub"
} > "$STATE/user-data"
printf 'instance-id: live-ubuntu-validation-20261028\nlocal-hostname: live-ubuntu-validation\n' > "$STATE/meta-data"
cloud-localds "$STATE/seed.img" "$STATE/user-data" "$STATE/meta-data"
qemu-img create -f qcow2 -F qcow2 -b "$BASE" "$STATE/disk.qcow2" 48G
printf '[127.0.0.1]:22428 ' > "$STATE/known_hosts"
cut -d ' ' -f 1,2 "$STATE/host.pub" >> "$STATE/known_hosts"
cat > "$STATE/ssh_config" <<EOF
Host live-ubuntu
  HostName 127.0.0.1
  Port 22428
  User validator
  IdentityFile $STATE/client
  IdentitiesOnly yes
  UserKnownHostsFile $STATE/known_hosts
  GlobalKnownHostsFile /dev/null
  HostKeyAlgorithms ssh-ed25519
  UpdateHostKeys no
  StrictHostKeyChecking yes
  PasswordAuthentication no
  BatchMode yes
  ConnectTimeout 5
  ServerAliveInterval 15
  ServerAliveCountMax 4
EOF
fi
export LIVE_UBUNTU_SSH_CONFIG="$STATE/ssh_config"
export LIVE_UBUNTU_STATE="$STATE"
vm_pid=
cleanup() {
  result=$?
  trap - EXIT INT TERM HUP
  if [ -n "$vm_pid" ]; then
    timeout 10 ssh -F "$STATE/ssh_config" live-ubuntu 'sudo poweroff' >/dev/null 2>&1 || true
    for ((i=0; i<30; i++)); do
      kill -0 "$vm_pid" 2>/dev/null || break
      sleep 1
    done
    kill -TERM "$vm_pid" 2>/dev/null || true
    for ((i=0; i<10; i++)); do
      kill -0 "$vm_pid" 2>/dev/null || break
      sleep 1
    done
    kill -KILL "$vm_pid" 2>/dev/null || true
    wait "$vm_pid" || true
  fi
  printf 'stopped_at=%s runner_exit=%s qemu_pid=%s run=%s\n' "$(date -u +%FT%TZ)" "$result" "$vm_pid" "$RUN_ID" | tee "$STATE/runtime-status.txt" >> "$STATE/runtime-history.txt"
  exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
qemu-system-x86_64 -name live-ubuntu-validation -machine q35,accel=kvm -cpu host \
  -smp 4 -m 8192 -display none -monitor none -no-reboot \
  -drive "file=$STATE/disk.qcow2,if=virtio,format=qcow2" \
  -drive "file=$STATE/seed.img,if=virtio,format=raw,readonly=on" \
  -netdev user,id=private,hostfwd=tcp:127.0.0.1:22428-:22 \
  -device virtio-net-pci,netdev=private -serial "file:$STATE/serial-$RUN_ID.log" \
  >"$STATE/qemu-$RUN_ID.log" 2>&1 &
vm_pid=$!
printf 'started_at=%s qemu_pid=%s acceleration=kvm vcpu=4 memory_mib=8192 mode=%s run=%s\n' "$(date -u +%FT%TZ)" "$vm_pid" "$MODE" "$RUN_ID" | tee "$STATE/runtime-status.txt" >> "$STATE/runtime-history.txt"
ready=0
for ((i=0; i<120; i++)); do
  kill -0 "$vm_pid" 2>/dev/null || { printf '%s\n' 'QEMU exited before SSH readiness.' >&2; exit 1; }
  if ssh -F "$STATE/ssh_config" live-ubuntu true 2>"$STATE/ssh-readiness.log"; then ready=1; break; fi
  sleep 2
done
[ "$ready" = 1 ] || { printf '%s\n' 'Pinned SSH readiness deadline exceeded.' >&2; exit 1; }
ssh -F "$STATE/ssh_config" live-ubuntu 'cloud-init status --wait; cat /etc/os-release; uname -a; systemd-detect-virt; nproc; free -m' | tee "$STATE/guest-inventory-$RUN_ID.txt"
"$@"
