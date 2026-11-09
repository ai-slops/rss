#!/usr/bin/env bash
# Run on the native Ubuntu 24.04 build host after ubuntu-dependencies.sh.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEST=${1:?Usage: bash scripts/package-server.sh NEW-OUTPUT-DIRECTORY}
test ! -e "$DEST" || { printf '%s\n' 'Output must be a new directory.' >&2; exit 1; }
source /etc/os-release
test "$ID" = ubuntu && test "$VERSION_ID" = 24.04
source "$ROOT/scripts/ubuntu-env.sh"
cd "$ROOT"
cargo build --locked --release -p live-production -p live-production-supervisor
(cd web && npm ci && npm run build)
mkdir -p "$DEST/app/bin" "$DEST/app/lib"
install -m755 target/release/live-production target/release/live-production-supervisor "$DEST/app/bin/"
install -m755 .tools/ubuntu/lib/gstreamer-1.0/libgstrswebrtc.so "$DEST/app/lib/"
cp -a web/dist "$DEST/app/web"
(cd "$DEST" && find app -type f -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS)
printf 'Container build context: %s\n' "$DEST"
