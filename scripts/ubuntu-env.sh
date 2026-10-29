#!/usr/bin/env bash
LIVE_UBUNTU_ROOT=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export LIVE_PRODUCTION_NATIVE=1
source "$LIVE_UBUNTU_ROOT/scripts/env.sh"
export PATH="$LIVE_UBUNTU_ROOT/.tools/node/bin:$HOME/.cargo/bin:$PATH"
export GST_PLUGIN_PATH="$LIVE_UBUNTU_ROOT/.tools/ubuntu/lib/gstreamer-1.0${GST_PLUGIN_PATH:+:$GST_PLUGIN_PATH}"
export GST_REGISTRY="$LIVE_UBUNTU_ROOT/.tools/ubuntu/registry.bin"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
