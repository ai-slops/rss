#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TOOLS_DIR=$PROJECT_ROOT/.tools
SYSROOT=$TOOLS_DIR/sysroot
STAGING=$TOOLS_DIR/sysroot.staging.$$
REPOSITORIES=$TOOLS_DIR/repositories
TOOLCHAIN_REVISION=alpine-3.23-rust-1.91.1-gstreamer-1.26.9-ffmpeg-8.0.1-v3

cleanup() {
  rm -rf "$STAGING"
}
trap cleanup EXIT HUP INT TERM

if [ "$(uname -m)" != x86_64 ]; then
  printf '%s\n' 'This bootstrap currently supports x86_64 only.' >&2
  exit 1
fi

if [ ! -x /sbin/apk ]; then
  printf '%s\n' 'This project-local bootstrap requires Alpine apk-tools on the execution host.' >&2
  exit 1
fi

mkdir -p "$TOOLS_DIR" "$TOOLS_DIR/bin" "$TOOLS_DIR/cache" "$TOOLS_DIR/cargo"
printf '%s\n' \
  'https://dl-cdn.alpinelinux.org/alpine/v3.23/main' \
  'https://dl-cdn.alpinelinux.org/alpine/v3.23/community' > "$REPOSITORIES"

installed_revision=$(cat "$TOOLS_DIR/revision" 2>/dev/null || true)
if [ "$installed_revision" != "$TOOLCHAIN_REVISION" ]; then
  rm -rf "$STAGING"
  mkdir -p "$STAGING"
  /sbin/apk \
    --root "$STAGING" \
    --usermode \
    --initdb \
    --keys-dir /etc/apk/keys \
    --repositories-file "$REPOSITORIES" \
    --no-scripts \
    add \
    build-base=0.5-r3 \
    cargo=1.91.1-r2 \
    rust=1.91.1-r2 \
    rust-clippy=1.91.1-r2 \
    rustfmt=1.91.1-r2 \
    pkgconf=2.5.1-r0 \
    gstreamer=1.26.9-r0 \
    gstreamer-dev=1.26.9-r0 \
    gstreamer-tools=1.26.9-r0 \
    gst-plugins-base=1.26.9-r0 \
    gst-plugins-base-dev=1.26.9-r0 \
    gst-plugins-good=1.26.9-r0 \
    gst-plugins-bad=1.26.9-r0 \
    gst-plugins-bad-dev=1.26.9-r0 \
    gst-plugins-ugly=1.26.9-r0 \
    gst-libav=1.26.9-r0 \
    gst-plugins-rs-rswebrtc=0.14.3-r0 \
    gst-plugins-rs-webrtchttp=0.14.3-r0 \
    libnice-gstreamer=0.1.22-r0 \
    ffmpeg=8.0.1-r1 \
    ffmpeg-dev=8.0.1-r1
  rm -rf "$SYSROOT"
  mv "$STAGING" "$SYSROOT"
fi

cat > "$TOOLS_DIR/bin/cc" <<'WRAPPER'
#!/bin/sh
set -eu
TOOLS_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SYSROOT=$TOOLS_DIR/sysroot
export LD_LIBRARY_PATH="$SYSROOT/usr/lib/pulseaudio:$SYSROOT/usr/lib:$SYSROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$SYSROOT/usr/bin/gcc" --sysroot="$SYSROOT" "$@"
WRAPPER

cat > "$TOOLS_DIR/bin/c++" <<'WRAPPER'
#!/bin/sh
set -eu
TOOLS_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SYSROOT=$TOOLS_DIR/sysroot
export LD_LIBRARY_PATH="$SYSROOT/usr/lib/pulseaudio:$SYSROOT/usr/lib:$SYSROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$SYSROOT/usr/bin/g++" --sysroot="$SYSROOT" "$@"
WRAPPER
chmod 755 "$TOOLS_DIR/bin/cc" "$TOOLS_DIR/bin/c++"

. "$PROJECT_ROOT/scripts/env.sh"

PROBE_DIR=$TOOLS_DIR/probe
mkdir -p "$PROBE_DIR"
cat > "$PROBE_DIR/gstreamer.c" <<'PROBE'
#include <gst/gst.h>

int main(int argc, char **argv) {
  gst_init(&argc, &argv);
  return gst_element_factory_find("compositor") == NULL;
}
PROBE

cat > "$PROBE_DIR/rust.rs" <<'PROBE'
fn main() {
    println!("local Rust compiler and linker work");
}
PROBE

pkg-config --atleast-version=1.26.9 gstreamer-1.0 gstreamer-video-1.0 gstreamer-audio-1.0
cc "$PROBE_DIR/gstreamer.c" -o "$PROBE_DIR/gstreamer-probe" $(pkg-config --cflags --libs gstreamer-1.0)
"$PROBE_DIR/gstreamer-probe"
cc "$PROJECT_ROOT/scripts/probes/glib-errors.c" -o "$PROBE_DIR/glib-errors" $(pkg-config --cflags --libs glib-2.0)
"$PROBE_DIR/glib-errors"
rustc "$PROBE_DIR/rust.rs" -o "$PROBE_DIR/rust-probe"
"$PROBE_DIR/rust-probe"
gst-launch-1.0 -q videotestsrc num-buffers=4 ! compositor ! fakesink
gst-launch-1.0 -q audiotestsrc num-buffers=4 ! audiomixer ! fakesink
gst-inspect-1.0 webrtcsink >/dev/null
gst-inspect-1.0 nicesrc >/dev/null
gst-inspect-1.0 whipclientsink >/dev/null
gst-inspect-1.0 whipsink >/dev/null
gst-inspect-1.0 whepsrc >/dev/null
ffmpeg -v error -f lavfi -i testsrc2=size=64x64:rate=1 -frames:v 1 -f null -
/sbin/apk --root "$SYSROOT" list --installed > "$TOOLS_DIR/apk-installed.txt"
printf '%s\n' "$TOOLCHAIN_REVISION" > "$TOOLS_DIR/revision"

printf '%s\n' "Toolchain ready. Activate with: . $PROJECT_ROOT/scripts/env.sh"
printf '%s\n' "Rust: $(rustc --version)"
printf '%s\n' "Cargo: $(cargo --version)"
printf '%s\n' "Rustfmt: $(rustfmt --version)"
printf '%s\n' "Clippy: $(cargo clippy --version)"
printf '%s\n' "GStreamer: $(gst-launch-1.0 --version | sed -n '1p')"
printf '%s\n' "FFmpeg: $(ffmpeg -version | sed -n '1p')"
