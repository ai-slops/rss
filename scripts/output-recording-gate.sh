#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$PROJECT_ROOT/scripts/smoke-supervisor.sh"
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/output-recording-gate}
BIND=${OUTPUT_GATE_BIND:-127.0.0.1:3099}
BASE_URL="http://$BIND/api/v1"

. "$PROJECT_ROOT/scripts/env.sh"
mkdir -p "$OUTPUT_DIR/state"

cleanup() {
  stop_owned_process "${server_pid:-}"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

CARGO_BUILD_JOBS=2 cargo build --locked -q --manifest-path "$PROJECT_ROOT/Cargo.toml" -p live-production

LIVE_PRODUCTION_BIND=$BIND \
LIVE_PRODUCTION_STATE_DIR="$OUTPUT_DIR/state" \
LIVE_PRODUCTION_PASSWORD=output-gate-password \
LIVE_PRODUCTION_SECURE_COOKIE=false \
LIVE_PRODUCTION_WEB_DIST="$PROJECT_ROOT/web/dist" \
"$PROJECT_ROOT/target/debug/live-production" >"$OUTPUT_DIR/server.log" 2>&1 &
server_pid=$!

attempt=0
until curl -sS --fail -c "$OUTPUT_DIR/cookies.txt" \
  -H 'x-live-production: 1' -H 'content-type: application/json' \
  -d '{"password":"output-gate-password"}' "$BASE_URL/auth/login" >/dev/null 2>&1; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 60 ]; then
    printf '%s\n' "server did not become ready" >&2
    exit 1
  fi
  sleep 0.25
done

api_get() {
  curl -sS --fail -b "$OUTPUT_DIR/cookies.txt" "$BASE_URL/$1"
}

api_command() {
  curl -sS --fail -b "$OUTPUT_DIR/cookies.txt" \
    -H 'x-live-production: 1' -H 'content-type: application/json' \
    -d "$1" "$BASE_URL/commands"
}

api_get snapshot >"$OUTPUT_DIR/initial.json"
project_id=$(python3 -c 'import json; print(json.load(open("'"$OUTPUT_DIR/initial.json"'"))["project"]["id"])')
engine_id=$(python3 -c 'import json; print(json.load(open("'"$OUTPUT_DIR/initial.json"'"))["engineRunId"])')

api_command "{\"id\":\"gate-source\",\"kind\":\"source.create\",\"targetId\":\"$project_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{\"name\":\"Clean fixture\",\"adapter\":{\"kind\":\"synthetic\",\"pattern\":\"ball\",\"frequencyHz\":550}}}" >/dev/null
api_get snapshot >"$OUTPUT_DIR/with-source.json"
source_id=$(python3 -c 'import json; print(json.load(open("'"$OUTPUT_DIR/with-source.json"'"))["project"]["sources"][0]["id"])')

api_command "{\"id\":\"gate-program-create\",\"kind\":\"output.create\",\"targetId\":\"$project_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{\"name\":\"Program archive\",\"adapter\":{\"kind\":\"programRecording\",\"segmentSeconds\":1}}}" >/dev/null
api_command "{\"id\":\"gate-clean-create\",\"kind\":\"output.create\",\"targetId\":\"$project_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{\"name\":\"Clean archive\",\"adapter\":{\"kind\":\"sourceRecording\",\"sourceId\":\"$source_id\",\"segmentSeconds\":1}}}" >/dev/null
api_get snapshot >"$OUTPUT_DIR/configured.json"
program_id=$(python3 -c 'import json; d=json.load(open("'"$OUTPUT_DIR/configured.json"'")); print(next(x["id"] for x in d["project"]["outputs"] if x["adapter"]["kind"]=="programRecording"))')
clean_id=$(python3 -c 'import json; d=json.load(open("'"$OUTPUT_DIR/configured.json"'")); print(next(x["id"] for x in d["project"]["outputs"] if x["adapter"]["kind"]=="sourceRecording"))')

api_command "{\"id\":\"gate-program-start\",\"kind\":\"output.start\",\"targetId\":\"$program_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{}}" >/dev/null
api_command "{\"id\":\"gate-clean-start\",\"kind\":\"output.start\",\"targetId\":\"$clean_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{}}" >/dev/null
sleep 3
api_command "{\"id\":\"gate-program-stop\",\"kind\":\"output.stop\",\"targetId\":\"$program_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{}}" >/dev/null
api_command "{\"id\":\"gate-clean-stop\",\"kind\":\"output.stop\",\"targetId\":\"$clean_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{}}" >/dev/null
api_get snapshot >"$OUTPUT_DIR/final-snapshot.json"

python3 -c 'import hashlib, json, pathlib, sys
snapshot=json.load(open(sys.argv[1]))
recordings=snapshot["recordings"]
assert len(recordings) >= 4, recordings
assert {item["tapPoint"] for item in recordings} == {"program", "clean_source"}
for item in recordings:
    path=pathlib.Path(item["path"])
    assert item["validation"] == "decoded", item
    assert path.stat().st_size == int(item["bytes"]), item
    assert hashlib.sha256(path.read_bytes()).hexdigest() == item["sha256"], item
print(f"recording_segments={len(recordings)}")' "$OUTPUT_DIR/final-snapshot.json"

for media_file in $(python3 -c 'import json,sys; print(" ".join(x["path"] for x in json.load(open(sys.argv[1]))["recordings"]))' "$OUTPUT_DIR/final-snapshot.json"); do
  ffprobe -v error -show_entries format=filename,duration,size \
    -show_entries stream=codec_name,codec_type,width,height,sample_rate,channels \
    -of compact=p=0:nk=0 "$media_file"
  ffmpeg -v error -xerror -i "$media_file" -f null -
done

printf '%s\n' "recording_gate=passed" "artifacts=$OUTPUT_DIR"
