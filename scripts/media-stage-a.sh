#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/media-stage-a}

. "$PROJECT_ROOT/scripts/env.sh"
export CARGO_BUILD_JOBS=3

mkdir -p "$OUTPUT_DIR"

cargo run -p live-production-media -- probe
cargo run -p live-production-media -- webrtc-gate --duration-seconds 3
cargo run -p live-production-media -- \
  stage-a \
  --output-dir "$OUTPUT_DIR" \
  --duration-seconds 6 \
  --holdback-ms 500 \
  --video-correction-ms=-250 \
  --audio-correction-ms 125

for media_file in "$OUTPUT_DIR"/*.mkv; do
  ffprobe \
    -v error \
    -show_entries format=filename,duration,size \
    -show_entries stream=codec_name,codec_type,width,height,sample_rate,channels \
    -of compact=p=0:nk=0 \
    "$media_file"
  ffmpeg -v error -xerror -i "$media_file" -f null -
done

ffprobe \
  -v error \
  -select_streams a \
  -show_entries packet=stream_index,pts_time \
  -of csv=p=0 \
  "$OUTPUT_DIR/timing-250ms.mkv" |
  python3 -c 'import sys
first = {}
for line in sys.stdin:
    stream, timestamp = line.strip().split(",")
    first.setdefault(int(stream), float(timestamp))
delta_ms = (first[1] - first[0]) * 1000
print(f"measured_timing_delta_ms={delta_ms:.3f}")
if abs(delta_ms - 250.0) > 1.0:
    raise SystemExit("timing delta outside 1 ms fixture tolerance")'
