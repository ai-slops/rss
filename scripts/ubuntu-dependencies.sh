#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [ "${1:-}" = --help ]; then
  printf '%s\n' 'Usage: bash scripts/ubuntu-dependencies.sh' 'Inside dedicated Ubuntu 24.04 only. Installs native packages and pinned project-local Node/rswebrtc.' 'Requires the installed Rust 1.91.1 toolchain. Public downloads are cached under ~/.cache/live-production.'
  exit 0
fi
source /etc/os-release
test "$ID" = ubuntu && test "$VERSION_ID" = 24.04
test "$(id -u)" != 0
source "$HOME/.cargo/env"
test "$(rustup run 1.91.1 rustc --version | cut -d ' ' -f 2)" = 1.91.1
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential pkg-config curl ca-certificates git xz-utils python3 file \
  libglib2.0-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  libgstreamer-plugins-bad1.0-dev libssl-dev qt6-base-dev \
  gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav \
  gstreamer1.0-nice gstreamer1.0-x ffmpeg obs-studio libobs-dev cmake xvfb xauth mesa-utils
umask 077
CACHE="$HOME/.cache/live-production"
DEST="$ROOT/.tools/ubuntu"
mkdir -p "$CACHE/src" "$DEST/lib/gstreamer-1.0" "$ROOT/.tools/node" "$ROOT/var/ubuntu-dependencies"
fetch() {
  local name=$1 digest=$2 url=$3
  if [ ! -f "$CACHE/$name" ]; then
    curl --fail --location --proto '=https' --tlsv1.2 "$url" -o "$CACHE/$name.partial"
    printf '%s  %s\n' "$digest" "$CACHE/$name.partial" | sha256sum -c -
    mv "$CACHE/$name.partial" "$CACHE/$name"
  fi
  printf '%s  %s\n' "$digest" "$CACHE/$name" | sha256sum -c -
}
NODE=node-v24.18.1-linux-x64
fetch "$NODE.tar.xz" d6c664df3f3f61458e8c277585571328522d705166723a7c7823a9253a4d15a0 \
  "https://nodejs.org/dist/v24.18.1/$NODE.tar.xz"
tar -xJf "$CACHE/$NODE.tar.xz" -C "$ROOT/.tools/node" --strip-components=1
CRATE=gst-plugin-webrtc-0.14.3
fetch "$CRATE.crate" 3e8b92068d633cdb6be65196f45d082e7beaf5aba1a00c825753a038fac2dfc6 \
  "https://static.crates.io/crates/gst-plugin-webrtc/$CRATE.crate"
if [ ! -f "$CACHE/src/$CRATE/Cargo.toml" ]; then
  tar -xzf "$CACHE/$CRATE.crate" -C "$CACHE/src"
fi
rustup run 1.91.1 cargo build --locked --release --lib --jobs 2 \
  --manifest-path "$CACHE/src/$CRATE/Cargo.toml"
install -m 755 "$CACHE/src/$CRATE/target/release/libgstrswebrtc.so" "$DEST/lib/gstreamer-1.0/"
source "$ROOT/scripts/ubuntu-env.sh"
gst-inspect-1.0 webrtcsink > "$ROOT/var/ubuntu-dependencies/webrtcsink.txt"
dpkg-query -W > "$ROOT/var/ubuntu-dependencies/packages.tsv"
sha256sum "$CACHE/src/$CRATE/Cargo.lock" "$DEST/lib/gstreamer-1.0/libgstrswebrtc.so" \
  > "$ROOT/var/ubuntu-dependencies/build-hashes.txt"
printf '%s\n' 'Native Ubuntu dependencies ready. Source scripts/ubuntu-env.sh before building and verification.'
