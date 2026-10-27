#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$PROJECT_ROOT/scripts/smoke-supervisor.sh"
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/output-gates}

. "$PROJECT_ROOT/scripts/env.sh"
mkdir -p "$OUTPUT_DIR"

cleanup() {
  for pid in ${gate_pid:-} ${proxy_one:-} ${proxy_two:-} ${receiver_one:-} ${receiver_two:-}; do
    stop_owned_process "$pid"
  done
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout "$OUTPUT_DIR/key.pem" -out "$OUTPUT_DIR/cert.pem" \
  -subj /CN=localhost -addext subjectAltName=DNS:localhost -days 1 >/dev/null 2>&1
cat "$SSL_CERT_FILE" "$OUTPUT_DIR/cert.pem" >"$OUTPUT_DIR/ca-bundle.pem"

run_pair() {
  phase=$1
  plain_one=$2
  plain_two=$3
  tls_one=$4
  tls_two=$5
  expected=$6
  duration=5
  if [ "$phase" = stall ]; then
    duration=15
  fi

  ffmpeg -nostdin -y -loglevel error -listen 1 \
    -i "rtmp://127.0.0.1:$plain_one/live/primary" -c copy -f flv \
    "$OUTPUT_DIR/$phase-primary.flv" >"$OUTPUT_DIR/$phase-primary-receiver.log" 2>&1 &
  receiver_one=$!
  ffmpeg -nostdin -y -loglevel error -listen 1 \
    -i "rtmp://127.0.0.1:$plain_two/live/secondary" -c copy -f flv \
    "$OUTPUT_DIR/$phase-secondary.flv" >"$OUTPUT_DIR/$phase-secondary-receiver.log" 2>&1 &
  receiver_two=$!
  python3 "$PROJECT_ROOT/scripts/output-tls-proxy.py" \
    --cert "$OUTPUT_DIR/cert.pem" --key "$OUTPUT_DIR/key.pem" \
    --listen-port "$tls_one" --target-port "$plain_one" &
  proxy_one=$!
  python3 "$PROJECT_ROOT/scripts/output-tls-proxy.py" \
    --cert "$OUTPUT_DIR/cert.pem" --key "$OUTPUT_DIR/key.pem" \
    --listen-port "$tls_two" --target-port "$plain_two" &
  proxy_two=$!
  sleep 1

  umask 077
  rm -f "$OUTPUT_DIR/$phase-ready"
  cat >"$OUTPUT_DIR/$phase-config.json" <<EOF
{"duration_seconds":$duration,"destinations":[
  {"id":"primary","url":"rtmps://localhost:$tls_one/live/primary"},
  {"id":"secondary","url":"rtmps://localhost:$tls_two/live/secondary"}
],"expected_unavailable":$expected,"ready_path":"$OUTPUT_DIR/$phase-ready"}
EOF

  SSL_CERT_FILE="$OUTPUT_DIR/ca-bundle.pem" CARGO_BUILD_JOBS=3 \
    cargo run -q -p live-production-media -- output-gate \
    --config "$OUTPUT_DIR/$phase-config.json" >"$OUTPUT_DIR/$phase-report.json" &
  gate_pid=$!
  attempt=0
  until [ -f "$OUTPUT_DIR/$phase-ready" ]; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 80 ]; then
      printf '%s\n' "output gate did not become ready" >&2
      exit 1
    fi
    sleep 0.25
  done
  if [ "$phase" = stall ]; then
    kill "$proxy_one"
    proxy_one=
  fi
  wait "$gate_pid"
  gate_pid=
  wait "$receiver_one" || true
  receiver_one=
  wait "$receiver_two"
  receiver_two=
  kill "$proxy_one" 2>/dev/null || true
  kill "$proxy_two" 2>/dev/null || true
  proxy_one=
  proxy_two=

  for media_file in "$OUTPUT_DIR/$phase-primary.flv" "$OUTPUT_DIR/$phase-secondary.flv"; do
    ffprobe -v error -show_entries format=filename,duration,size \
      -show_entries stream=codec_name,codec_type,width,height,sample_rate,channels \
      -of compact=p=0:nk=0 "$media_file"
    ffmpeg -v error -xerror -i "$media_file" -f null -
  done
}

run_pair healthy 19370 19371 19470 19471 '[]'
run_pair stall 19372 19373 19472 19473 '["primary"]'

sha256sum "$OUTPUT_DIR"/*.flv >"$OUTPUT_DIR/sha256.txt"
printf '%s\n' "output_gates=passed" "artifacts=$OUTPUT_DIR"
