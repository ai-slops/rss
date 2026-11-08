#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$PROJECT_ROOT/scripts/smoke-supervisor.sh"
SRT_PORT=${SRT_PORT:-$(node "$PROJECT_ROOT/scripts/free-port.mjs" udp)}
RTMP_PORT=${RTMP_PORT:-$(node "$PROJECT_ROOT/scripts/free-port.mjs" tcp)}

. "$PROJECT_ROOT/scripts/env.sh"

mkdir -p "$PROJECT_ROOT/var"
OUTPUT_DIR=${1:-$(mktemp -d "$PROJECT_ROOT/var/media-network.XXXXXX")}
mkdir -p "$OUTPUT_DIR"
srt_receiver=
rtmp_receiver=
cleanup() {
  stop_owned_process "$srt_receiver"
  stop_owned_process "$rtmp_receiver"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

timeout 12 gst-launch-1.0 -e -q \
  srtsrc uri="srt://127.0.0.1:$SRT_PORT?mode=listener" latency=125 keep-listening=false blocksize=4096 wait-for-connection=true \
  ! filesink location="$OUTPUT_DIR/srt-received.ts" &
srt_receiver=$!
sleep 1
timeout 10 gst-launch-1.0 -e -q \
  mpegtsmux name=mux alignment=7 \
  ! tee name=srt_out \
  srt_out. ! queue ! srtsink uri="srt://127.0.0.1:$SRT_PORT?mode=caller" latency=125 wait-for-connection=true \
  srt_out. ! queue ! fakesink sync=true ts-offset="${SRT_DRAIN_NS:-500000000}" \
  videotestsrc is-live=true num-buffers=150 \
  ! video/x-raw,width=640,height=360,framerate=30/1 \
  ! videoconvert \
  ! video/x-raw,format=I420 \
  ! x264enc tune=zerolatency key-int-max=30 bitrate=1000 byte-stream=true option-string=repeat-headers=1 \
  ! video/x-h264,stream-format=byte-stream,alignment=au \
  ! h264parse config-interval=-1 disable-passthrough=true \
  ! mux. \
  audiotestsrc is-live=true num-buffers=235 \
  ! audioconvert \
  ! audioresample \
  ! audio/x-raw,rate=48000,channels=2 \
  ! voaacenc bitrate=128000 \
  ! aacparse \
  ! mux.
wait "$srt_receiver"
srt_receiver=
ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of json "$OUTPUT_DIR/srt-received.ts" |
  python3 -c 'import json, sys
frames = int(json.load(sys.stdin)["streams"][0]["nb_read_frames"])
print(f"srt_decoded_video_frames={frames}")
if frames != 150:
    raise SystemExit("SRT fixture lost video frames")'

timeout 15 ffmpeg \
  -nostdin \
  -loglevel error \
  -listen 1 \
  -i "rtmp://127.0.0.1:$RTMP_PORT/live/test" \
  -c copy \
  -f flv \
  "$OUTPUT_DIR/rtmp-received.flv" &
rtmp_receiver=$!
sleep 1
timeout -s INT 5 gst-launch-1.0 -e -q \
  flvmux name=mux streamable=true \
  ! rtmpsink location="rtmp://127.0.0.1:$RTMP_PORT/live/test" \
  videotestsrc is-live=true \
  ! video/x-raw,width=640,height=360,framerate=30/1 \
  ! videoconvert \
  ! x264enc tune=zerolatency key-int-max=30 bitrate=1000 \
  ! h264parse \
  ! mux. \
  audiotestsrc is-live=true \
  ! audioconvert \
  ! audioresample \
  ! audio/x-raw,rate=48000,channels=2 \
  ! voaacenc bitrate=128000 \
  ! aacparse \
  ! mux. || test "$?" -eq 124
wait "$rtmp_receiver" || true
rtmp_receiver=

for media_file in "$OUTPUT_DIR/srt-received.ts" "$OUTPUT_DIR/rtmp-received.flv"; do
  test "$(ffprobe -v error -select_streams v:0 -show_entries stream=codec_name,width,height -of csv=p=0 "$media_file" | sed -n '1p')" = "h264,640,360"
  test "$(ffprobe -v error -select_streams a:0 -show_entries stream=sample_rate,channels -of csv=p=0 "$media_file" | sed -n '1p')" = "48000,2"
  ffprobe \
    -v error \
    -show_entries format=filename,duration,size \
    -show_entries stream=codec_name,codec_type,width,height,sample_rate,channels \
    -of compact=p=0:nk=0 \
    "$media_file"
  ffmpeg -v error -xerror -i "$media_file" -f null -
done
