#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$PROJECT_ROOT/scripts/smoke-supervisor.sh"
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/output-shutdown-gate}
BIND=${OUTPUT_SHUTDOWN_BIND:-127.0.0.1:3098}
BASE_URL="http://$BIND/api/v1"

. "$PROJECT_ROOT/scripts/env.sh"
mkdir -p "$OUTPUT_DIR/state"
CARGO_BUILD_JOBS=3 cargo build -q -p live-production

cleanup() {
  stop_owned_process "${server_pid:-}"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

start_server() {
  LIVE_PRODUCTION_BIND=$BIND \
  LIVE_PRODUCTION_STATE_DIR="$OUTPUT_DIR/state" \
  LIVE_PRODUCTION_PASSWORD=output-shutdown-password \
  LIVE_PRODUCTION_SECURE_COOKIE=false \
  LIVE_PRODUCTION_WEB_DIST="$PROJECT_ROOT/web/dist" \
  "$PROJECT_ROOT/target/debug/live-production" >"$OUTPUT_DIR/server-$1.log" 2>&1 &
  server_pid=$!
  attempt=0
  until curl -sS --fail -c "$OUTPUT_DIR/cookies.txt" \
    -H 'x-live-production: 1' -H 'content-type: application/json' \
    -d '{"password":"output-shutdown-password"}' "$BASE_URL/auth/login" >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 60 ]; then
      printf '%s\n' "server did not become ready" >&2
      exit 1
    fi
    sleep 0.25
  done
}

snapshot() {
  curl -sS --fail -b "$OUTPUT_DIR/cookies.txt" "$BASE_URL/snapshot"
}

command() {
  curl -sS --fail -b "$OUTPUT_DIR/cookies.txt" \
    -H 'x-live-production: 1' -H 'content-type: application/json' \
    -d "$1" "$BASE_URL/commands"
}

start_server first
snapshot >"$OUTPUT_DIR/initial.json"
project_id=$(python3 -c 'import json; print(json.load(open("'"$OUTPUT_DIR/initial.json"'"))["project"]["id"])')
engine_id=$(python3 -c 'import json; print(json.load(open("'"$OUTPUT_DIR/initial.json"'"))["engineRunId"])')
command "{\"id\":\"shutdown-create\",\"kind\":\"output.create\",\"targetId\":\"$project_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{\"name\":\"Shutdown archive\",\"adapter\":{\"kind\":\"programRecording\",\"segmentSeconds\":1}}}" >/dev/null
snapshot >"$OUTPUT_DIR/configured.json"
output_id=$(python3 -c 'import json; print(json.load(open("'"$OUTPUT_DIR/configured.json"'"))["project"]["outputs"][0]["id"])')
command "{\"id\":\"shutdown-start\",\"kind\":\"output.start\",\"targetId\":\"$output_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{}}" >/dev/null
sleep 3

kill -TERM "$server_pid"
wait "$server_pid"
server_pid=

catalog=$(python3 -c 'import pathlib,sys; files=list(pathlib.Path(sys.argv[1]).glob("recordings/*/*/catalog.json")); assert len(files)==1, files; print(files[0])' "$OUTPUT_DIR/state")
python3 -c 'import json,sys; items=json.load(open(sys.argv[1])); assert items and all(item["decoded"] for item in items); print(f"shutdown_segments={len(items)}")' "$catalog"

start_server restarted
snapshot >"$OUTPUT_DIR/restarted-snapshot.json"
python3 -c 'import json,sys
snapshot=json.load(open(sys.argv[1]))
assert snapshot["outputStatuses"][0]["desired"] == "stopped", snapshot["outputStatuses"]
assert snapshot["outputStatuses"][0]["actual"] == "stopped", snapshot["outputStatuses"]
assert snapshot["recordings"] and all(item["validation"] == "decoded" for item in snapshot["recordings"])
print(f"restored_segments={len(snapshot['"'"'recordings'"'"'])}")' "$OUTPUT_DIR/restarted-snapshot.json"

kill -TERM "$server_pid"
wait "$server_pid"
server_pid=
printf '%s\n' "shutdown_gate=passed" "artifacts=$OUTPUT_DIR"
