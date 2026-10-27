#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/media-network-gates}
SRT_PORT=${SRT_PORT:-19360}
RTMP_PORT=${RTMP_PORT:-19350}

. "$PROJECT_ROOT/scripts/env.sh"

mkdir -p "$OUTPUT_DIR"

timeout 12 gst-launch-1.0 -e -q \
  srtsrc uri="srt://:$SRT_PORT?mode=listener" wait-for-connection=true \
  ! filesink location="$OUTPUT_DIR/srt-received.ts" &
srt_receiver=$!
sleep 1
timeout -s INT 5 gst-launch-1.0 -e -q \
  mpegtsmux name=mux \
  ! srtsink uri="srt://127.0.0.1:$SRT_PORT?mode=caller" wait-for-connection=true \
  videotestsrc is-live=true \
  ! video/x-raw,width=640,height=360,framerate=30/1 \
  ! videoconvert \
  ! x264enc tune=zerolatency key-int-max=30 bitrate=1000 \
  ! h264parse config-interval=-1 \
  ! mux. \
  audiotestsrc is-live=true \
  ! audioconvert \
  ! audioresample \
  ! voaacenc bitrate=128000 \
  ! aacparse \
  ! mux. || test "$?" -eq 124
wait "$srt_receiver" || test "$?" -eq 124

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
  ! voaacenc bitrate=128000 \
  ! aacparse \
  ! mux. || test "$?" -eq 124
wait "$rtmp_receiver" || true

for media_file in "$OUTPUT_DIR/srt-received.ts" "$OUTPUT_DIR/rtmp-received.flv"; do
  ffprobe \
    -v error \
    -show_entries format=filename,duration,size \
    -show_entries stream=codec_name,codec_type,width,height,sample_rate,channels \
    -of compact=p=0:nk=0 \
    "$media_file"
  ffmpeg -v error -xerror -i "$media_file" -f null -
done
