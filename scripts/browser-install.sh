#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BROWSER_ROOT=$PROJECT_ROOT/.tools/browser-root
REPOSITORIES=$PROJECT_ROOT/.tools/repositories
CHROMIUM_VERSION=149.0.7827.53-r0
CJK_FONT_VERSION=0_git20220127-r1

if { [ -x "$BROWSER_ROOT/usr/bin/chromium-browser" ] || [ -x "$BROWSER_ROOT/usr/bin/chromium" ]; } && [ -f "$BROWSER_ROOT/usr/share/fonts/noto/NotoSansCJK-Regular.ttc" ]; then
  exit 0
fi

mkdir -p "$BROWSER_ROOT"
/sbin/apk \
  --root "$BROWSER_ROOT" \
  --usermode \
  --initdb \
  --keys-dir /etc/apk/keys \
  --repositories-file "$REPOSITORIES" \
  --no-scripts \
  add "chromium=$CHROMIUM_VERSION" "font-noto-cjk=$CJK_FONT_VERSION"
