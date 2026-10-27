#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CHROMIUM=$PROJECT_ROOT/.tools/browser-root/usr/bin/chromium-browser
if [ ! -x "$CHROMIUM" ]; then
  CHROMIUM=$PROJECT_ROOT/.tools/browser-root/usr/bin/chromium
fi
if [ ! -x "$CHROMIUM" ]; then
  printf '%s\n' "Project-local Chromium is missing. Run: $PROJECT_ROOT/scripts/browser-install.sh" >&2
  exit 1
fi

export LD_LIBRARY_PATH="$PROJECT_ROOT/.tools/browser-root/usr/lib/pulseaudio:$PROJECT_ROOT/.tools/browser-root/usr/lib:$PROJECT_ROOT/.tools/browser-root/lib"
if [ "${LD_PRELOAD:-}" = /lib/libgcompat.so.0 ] || [ "${LD_PRELOAD:-}" = libgcompat.so.0 ]; then unset LD_PRELOAD; fi
export FONTCONFIG_PATH="$PROJECT_ROOT/.tools/browser-root/etc/fonts"
export FONTCONFIG_SYSROOT="$PROJECT_ROOT/.tools/browser-root"
export XDG_CACHE_HOME="$PROJECT_ROOT/.tools/cache"
export XDG_DATA_DIRS="$PROJECT_ROOT/.tools/browser-root/usr/share"
exec "$CHROMIUM" --no-sandbox --disable-dev-shm-usage "$@"
