#!/usr/bin/env bash
# Package a CMake stage for OBS's per-user Linux plugin directory.
set -euo pipefail
if [ "$#" != 2 ]; then
  printf '%s\n' 'Usage: bash scripts/package-obs-plugin.sh STAGE OUTPUT.tar.gz' >&2
  exit 2
fi
STAGE=$(realpath "$1")
OUTPUT=$(realpath -m "$2")
test -f "$STAGE/bin/64bit/obs-live-production.so"
test -x "$STAGE/bin/64bit/obs-live-production-srt"
test -d "$STAGE/data/obs-plugins/obs-live-production"
test ! -e "$OUTPUT"
tar -czf "$OUTPUT" \
  --transform='s,^bin/,obs-live-production/bin/,' \
  --transform='s,^data/obs-plugins/obs-live-production,obs-live-production/data,' \
  -C "$STAGE" bin/64bit/obs-live-production.so bin/64bit/obs-live-production-srt data/obs-plugins/obs-live-production
sha256sum "$OUTPUT"
