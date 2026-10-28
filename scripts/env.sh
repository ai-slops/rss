#!/bin/sh

if [ -n "${BASH_SOURCE:-}" ]; then
  _live_production_env_source=$BASH_SOURCE
else
  _live_production_env_source=$0
fi

LIVE_PRODUCTION_ROOT=$(CDPATH= cd -- "$(dirname -- "$_live_production_env_source")/.." && pwd)
if [ "${LIVE_PRODUCTION_NATIVE:-0}" = 1 ]; then
  # Explicit native-host verification, never an implicit Alpine fallback.
  if [ -n "${PKG_CONFIG_SYSROOT_DIR:-}${LIVE_PRODUCTION_SYSROOT:-}" ]; then
    printf '%s\n' 'Native verification refuses an inherited sysroot.' >&2
    return 1 2>/dev/null || exit 1
  fi
  export LIVE_PRODUCTION_ROOT
  export RUST_LOG="${RUST_LOG:-info},gst_plugin_webrtc_signalling=warn"
  unset _live_production_env_source
  return 0 2>/dev/null || exit 0
fi
LIVE_PRODUCTION_SYSROOT=$LIVE_PRODUCTION_ROOT/.tools/sysroot

if [ ! -x "$LIVE_PRODUCTION_SYSROOT/usr/bin/rustc" ]; then
  printf '%s\n' "Local toolchain is missing. Run: $LIVE_PRODUCTION_ROOT/scripts/bootstrap.sh" >&2
  return 1 2>/dev/null || exit 1
fi

export LIVE_PRODUCTION_ROOT LIVE_PRODUCTION_SYSROOT
export PATH="$LIVE_PRODUCTION_ROOT/.tools/bin:$LIVE_PRODUCTION_SYSROOT/usr/bin:$LIVE_PRODUCTION_SYSROOT/bin:$PATH"
export LD_LIBRARY_PATH="$LIVE_PRODUCTION_SYSROOT/usr/lib/pulseaudio:$LIVE_PRODUCTION_SYSROOT/usr/lib:$LIVE_PRODUCTION_SYSROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PKG_CONFIG_SYSROOT_DIR="$LIVE_PRODUCTION_SYSROOT"
export PKG_CONFIG_LIBDIR="$LIVE_PRODUCTION_SYSROOT/usr/lib/pkgconfig:$LIVE_PRODUCTION_SYSROOT/usr/share/pkgconfig"
export GST_PLUGIN_PATH="$LIVE_PRODUCTION_SYSROOT/usr/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER="$LIVE_PRODUCTION_SYSROOT/usr/libexec/gstreamer-1.0/gst-plugin-scanner"
export GST_REGISTRY="$LIVE_PRODUCTION_ROOT/.tools/cache/gstreamer-registry.bin"
export GIO_USE_NETWORK_MONITOR=base
export GSETTINGS_BACKEND=memory
export FONTCONFIG_PATH="$LIVE_PRODUCTION_SYSROOT/etc/fonts"
export XDG_DATA_DIRS="$LIVE_PRODUCTION_SYSROOT/usr/share${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
export SSL_CERT_FILE="$LIVE_PRODUCTION_SYSROOT/etc/ssl/certs/ca-certificates.crt"
export CARGO_HOME="$LIVE_PRODUCTION_ROOT/.tools/cargo"
export CC="$LIVE_PRODUCTION_ROOT/.tools/bin/cc"
export CXX="$LIVE_PRODUCTION_ROOT/.tools/bin/c++"
export AR="$LIVE_PRODUCTION_SYSROOT/usr/bin/ar"
export RUST_LOG="${RUST_LOG:-info},gst_plugin_webrtc_signalling=warn"
if [ "${LD_PRELOAD:-}" = /lib/libgcompat.so.0 ] || [ "${LD_PRELOAD:-}" = libgcompat.so.0 ]; then
  unset LD_PRELOAD
fi

unset _live_production_env_source
