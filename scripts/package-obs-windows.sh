#!/usr/bin/env bash
# Package a tested Windows CMake stage; no Windows binary is built on Linux.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PACKAGE=$(realpath "${1:?Usage: bash scripts/package-obs-windows.sh PLUGIN-DIRECTORY OUTPUT.exe}")
OUTPUT=$(realpath -m "${2:?Output executable is required}")
test ! -e "$OUTPUT"
for binary in obs-live-production.dll obs-live-production-srt.exe srt.dll; do
  test -s "$PACKAGE/bin/64bit/$binary"
done
test -s "$PACKAGE/data/locale/en-US.ini"
mkdir -p "$(dirname "$OUTPUT")"
if ! docker image inspect live-production-nsis:local >/dev/null 2>&1; then
  docker build -t live-production-nsis:local -f "$ROOT/deploy/nsis.Dockerfile" "$ROOT/deploy"
fi
docker run --rm --user "$(id -u):$(id -g)" \
  -v "$PACKAGE:/package:ro" -v "$(dirname "$OUTPUT"):/out" \
  -v "$ROOT/integrations/obs-live-production/installer:/installer:ro" \
  live-production-nsis:local makensis -V2 -DPACKAGE=/package \
  "-DOUTPUT=/out/$(basename "$OUTPUT")" /installer/windows.nsi
