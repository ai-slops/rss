#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
if [ -n "${LIVE_PRODUCTION_SYSROOT:-}" ] || [ -n "${PKG_CONFIG_SYSROOT_DIR:-}" ]; then
  printf '%s\n' "Refusing project-local sysroot variables on native Ubuntu preflight." >&2
  exit 1
fi
case "${LD_LIBRARY_PATH:-}:${GST_PLUGIN_PATH:-}" in
  *".tools/sysroot"*) printf '%s\n' "Refusing Alpine project sysroot library/plugin paths." >&2; exit 1 ;;
esac
exec node "$ROOT/scripts/target/preflight.mjs" "$@"
