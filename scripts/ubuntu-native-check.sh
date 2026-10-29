#!/usr/bin/env bash
# Run as the disposable guest's unprivileged validator from a clean clone.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
mkdir -p var/ubuntu-native
exec > >(tee var/ubuntu-native/session.log) 2>&1
printf 'started_at=%s source=%s\n' "$(date -u +%FT%TZ)" "$(GIT_MASTER=1 git rev-parse HEAD)"
test -z "$(GIT_MASTER=1 git status --porcelain)"
test ! -e .tools/sysroot
source /etc/os-release
test "$ID" = ubuntu && test "$VERSION_ID" = 24.04
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential pkg-config curl ca-certificates git xz-utils python3 \
  libglib2.0-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav \
  gstreamer1.0-nice ffmpeg obs-studio libobs-dev cmake xvfb xauth mesa-utils
dpkg-query -W > var/ubuntu-native/packages.tsv
curl --fail --location --proto '=https' --tlsv1.2 https://sh.rustup.rs -o var/ubuntu-native/rustup-init.sh
sha256sum var/ubuntu-native/rustup-init.sh
sh var/ubuntu-native/rustup-init.sh -y --profile minimal --default-toolchain 1.91.1 --component rustfmt,clippy
source "$HOME/.cargo/env"
NODE=node-v24.18.1-linux-x64
curl --fail --location "https://nodejs.org/dist/v24.18.1/$NODE.tar.xz" -o "var/ubuntu-native/$NODE.tar.xz"
curl --fail --location https://nodejs.org/dist/v24.18.1/SHASUMS256.txt -o var/ubuntu-native/node-shasums.txt
(cd var/ubuntu-native; sha256sum -c --ignore-missing node-shasums.txt)
mkdir -p .tools/node
tar -xJf "var/ubuntu-native/$NODE.tar.xz" -C .tools/node --strip-components=1
export PATH="$ROOT/.tools/node/bin:$PATH"
export LIVE_PRODUCTION_NATIVE=1 CARGO_BUILD_JOBS=2
source scripts/env.sh
{
  rustc --version; cargo --version; cc --version; node --version; npm --version
  pkg-config --modversion gstreamer-1.0 gstreamer-app-1.0 gio-2.0 libobs
  ffmpeg -version; obs --version
} > var/ubuntu-native/versions.txt
cat var/ubuntu-native/versions.txt
: > var/ubuntu-native/elements.tsv
failed=0
for element in compositor audiomixer x264enc voaacenc mpegtsmux splitmuxsink srtsrc srtsink rtmpsink webrtcsink; do
  if gst-inspect-1.0 "$element" > "var/ubuntu-native/element-$element.txt" 2>&1; then
    printf '%s\tavailable\n' "$element" | tee -a var/ubuntu-native/elements.tsv
  else
    printf '%s\tMISSING\n' "$element" | tee -a var/ubuntu-native/elements.tsv
    failed=1
  fi
done
gate() {
  local name=$1 result=0
  shift
  printf '\nGATE %s started_at=%s\n' "$name" "$(date -u +%FT%TZ)"
  "$@" > "var/ubuntu-native/$name.log" 2>&1 || result=$?
  printf '%s\t%s\n' "$name" "$result" | tee -a var/ubuntu-native/results.tsv
  [ "$result" = 0 ] || { failed=1; tail -n 60 "var/ubuntu-native/$name.log"; }
}
: > var/ubuntu-native/results.tsv
gate npm-ci npm --prefix web ci --no-fund --no-audit
gate cargo-build cargo build --locked --workspace
if [ -x target/debug/live-production-supervisor ]; then
  gate rust-fmt cargo fmt --all -- --check
  gate rust-clippy cargo clippy --locked --workspace --all-targets -- -D warnings
  gate rust-tests target/debug/live-production-supervisor --timeout-ms 900000 --grace-ms 10000 -- cargo test --locked --workspace
  gate lifecycle bash scripts/lifecycle-acceptance.sh
  gate media-stage-a bash scripts/media-stage-a.sh
  gate media-network bash scripts/media-network-gates.sh
else
  printf 'supervisor-prerequisite\t1\n' | tee -a var/ubuntu-native/results.tsv
  failed=1
fi
gate web-check npm --prefix web run check
gate web-tests npm --prefix web test
gate web-build npm --prefix web run build
gate npm-audit npm --prefix web audit --audit-level=moderate
mkdir -p var/ubuntu-native/obs-config var/ubuntu-native/xdg-runtime
chmod 700 var/ubuntu-native/xdg-runtime
obs_result=0
XDG_CONFIG_HOME="$ROOT/var/ubuntu-native/obs-config" \
  XDG_RUNTIME_DIR="$ROOT/var/ubuntu-native/xdg-runtime" LIBGL_ALWAYS_SOFTWARE=1 \
  timeout --signal=INT --kill-after=10 20 xvfb-run -a obs --multi --verbose \
  --disable-shutdown-check > var/ubuntu-native/obs-startup.log 2>&1 || obs_result=$?
if [ "$obs_result" = 124 ] && grep -q 'Startup complete' var/ubuntu-native/obs-startup.log; then
  printf 'obs-software-startup\t0\n' | tee -a var/ubuntu-native/results.tsv
else
  printf 'obs-software-startup\t%s\n' "$obs_result" | tee -a var/ubuntu-native/results.tsv
  failed=1
fi
printf 'finished_at=%s failed=%s\n' "$(date -u +%FT%TZ)" "$failed"
exit "$failed"
