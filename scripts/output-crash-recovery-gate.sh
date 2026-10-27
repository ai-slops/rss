#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$PROJECT_ROOT/scripts/env.sh"

SUPERVISOR=$PROJECT_ROOT/target/debug/live-production-supervisor
APP=${LIVE_PRODUCTION_TEST_BINARY:-${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}/debug/live-production}
OUTPUT_DIR=${1:-$(mktemp -d "$PROJECT_ROOT/var/output-crash-recovery.XXXXXX")}

if [ "${LIVE_PRODUCTION_CRASH_SCOPE:-}" != "$0" ]; then
  exec "$SUPERVISOR" --timeout-ms 180000 --grace-ms 10000 -- \
    env LIVE_PRODUCTION_CRASH_SCOPE="$0" LIVE_PRODUCTION_TEST_BINARY="$APP" \
    bash "$0" "$OUTPUT_DIR"
fi

test -x "$APP"
case "$OUTPUT_DIR" in
  "$PROJECT_ROOT"/var/*|var/*) ;;
  *) printf '%s\n' "crash evidence must stay under $PROJECT_ROOT/var" >&2; exit 1 ;;
esac
umask 077
mkdir -p "$OUTPUT_DIR/state"

app_pid=
app_stopped=false
cleanup() {
  if [ -n "$app_pid" ] && kill -0 "$app_pid" 2>/dev/null; then
    if [ "$app_stopped" = true ]; then kill -CONT "$app_pid" 2>/dev/null || true; fi
    kill -TERM "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  rm -f "$OUTPUT_DIR/first-cookies.txt" "$OUTPUT_DIR/restarted-cookies.txt"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

free_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'
}

start_app() {
  phase=$1
  LISTEN_PORT=$(free_port)
  printf 'http://127.0.0.1:%s/api/v1\n' "$LISTEN_PORT" >"$OUTPUT_DIR/current-base-url.txt"
  printf '%s\n' "$LISTEN_PORT" >"$OUTPUT_DIR/$phase-port.txt"
  LIVE_PRODUCTION_BIND="127.0.0.1:$LISTEN_PORT" \
  LIVE_PRODUCTION_STATE_DIR="$OUTPUT_DIR/state" \
  LIVE_PRODUCTION_PASSWORD=crash-recovery-fixture-password \
  LIVE_PRODUCTION_SECURE_COOKIE=false \
  "$APP" >"$OUTPUT_DIR/$phase-server.log" 2>&1 &
  app_pid=$!
  app_stopped=false
  attempt=0
  until curl -sS --fail -c "$OUTPUT_DIR/$phase-cookies.txt" \
    -H 'x-live-production: 1' -H 'content-type: application/json' \
    -d '{"password":"crash-recovery-fixture-password"}' \
    "$(cat "$OUTPUT_DIR/current-base-url.txt")/auth/login" >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 120 ]; then
      curl -v --max-time 2 -H 'x-live-production: 1' \
        -H 'content-type: application/json' \
        -d '{"password":"crash-recovery-fixture-password"}' \
        "$(cat "$OUTPUT_DIR/current-base-url.txt")/auth/login" >&2 || true
      printf '%s\n' "isolated daemon did not become ready" >&2
      exit 1
    fi
    sleep 0.1
  done
  COOKIE_FILE=$OUTPUT_DIR/$phase-cookies.txt
}

snapshot() {
  curl -sS --fail -b "$COOKIE_FILE" "$(cat "$OUTPUT_DIR/current-base-url.txt")/snapshot"
}

command() {
  curl -sS --fail -b "$COOKIE_FILE" \
    -H 'x-live-production: 1' -H 'content-type: application/json' \
    -d "$1" "$(cat "$OUTPUT_DIR/current-base-url.txt")/commands"
}

start_app first
snapshot >"$OUTPUT_DIR/initial.json"
project_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["project"]["id"])' "$OUTPUT_DIR/initial.json")
engine_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["engineRunId"])' "$OUTPUT_DIR/initial.json")
command "{\"id\":\"crash-output-create\",\"kind\":\"output.create\",\"targetId\":\"$project_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{\"name\":\"Crash archive\",\"adapter\":{\"kind\":\"programRecording\",\"segmentSeconds\":1}}}" >/dev/null
snapshot >"$OUTPUT_DIR/configured.json"
output_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["project"]["outputs"][0]["id"])' "$OUTPUT_DIR/configured.json")
command "{\"id\":\"crash-output-start\",\"kind\":\"output.start\",\"targetId\":\"$output_id\",\"expectedEngineRunId\":\"$engine_id\",\"payload\":{}}" >/dev/null

attempt=0
while :; do
  raw_count=$(find "$OUTPUT_DIR/state/recordings" -name 'segment-*.mkv' -type f 2>/dev/null | wc -l)
  closed_count=$(python3 - "$OUTPUT_DIR/state/recordings" <<'PY'
import json,pathlib,sys
catalogs=list(pathlib.Path(sys.argv[1]).glob("*/*/catalog.json"))
count=0
for path in catalogs:
    try: count += len(json.loads(path.read_text()))
    except (OSError,ValueError,TypeError): pass
print(count)
PY
)
  if [ "$raw_count" -ge 3 ] && [ "$closed_count" -ge 2 ]; then break; fi
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 180 ]; then
    printf '%s\n' "recorder did not publish three raw and two closed fragments" >&2
    exit 1
  fi
  sleep 0.1
done
cp "$(find "$OUTPUT_DIR/state/recordings" -name catalog.json -type f | sed -n '1p')" \
  "$OUTPUT_DIR/catalog-before-kill.json"
kill -STOP "$app_pid"
app_stopped=true
attempt=0
while ! grep -q '^State:[[:space:]]*T' "/proc/$app_pid/status" 2>/dev/null; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 50 ]; then
    printf '%s\n' "isolated daemon did not stop before hashing" >&2
    exit 1
  fi
  sleep 0.05
done
find "$OUTPUT_DIR/state/recordings" -name 'segment-*.mkv' -type f -print0 \
  | sort -z | xargs -0 sha256sum >"$OUTPUT_DIR/before.sha256"

kill -KILL "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=
app_stopped=false

find "$OUTPUT_DIR/state/recordings" -name 'segment-*.mkv' -type f -print0 \
  | sort -z | xargs -0 sha256sum >"$OUTPUT_DIR/after-kill.sha256"
cmp "$OUTPUT_DIR/before.sha256" "$OUTPUT_DIR/after-kill.sha256"

start_app restarted
snapshot >"$OUTPUT_DIR/restarted-snapshot.json"
new_engine_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["engineRunId"])' "$OUTPUT_DIR/restarted-snapshot.json")
command "{\"id\":\"crash-editor-start\",\"kind\":\"plugin.command\",\"targetId\":\"editor-session\",\"expectedEngineRunId\":\"$new_engine_id\",\"payload\":{\"command\":\"session.start\",\"text\":\"abrupt-recovery\"}}" >/dev/null
command "{\"id\":\"crash-editor-export\",\"kind\":\"plugin.command\",\"targetId\":\"editor-session\",\"expectedEngineRunId\":\"$new_engine_id\",\"payload\":{\"command\":\"session.export\"}}" >"$OUTPUT_DIR/export.json"

python3 - "$OUTPUT_DIR/restarted-snapshot.json" "$OUTPUT_DIR/export.json" "$OUTPUT_DIR/before.sha256" "$OUTPUT_DIR/initial.json" "$OUTPUT_DIR/catalog-before-kill.json" <<'PY'
import hashlib, json, pathlib, sys
snapshot=json.load(open(sys.argv[1])); exported=json.load(open(sys.argv[2])); initial=json.load(open(sys.argv[4])); expected={}
for line in open(sys.argv[3]):
    digest, path=line.rstrip().split("  ",1); expected[path]=digest
recordings=snapshot["recordings"]
assert recordings, "restart hid all fragments"
assert len(recordings)==len(expected), (len(recordings),len(expected))
assert len(recordings)>=3, recordings
assert snapshot["outputStatuses"][0]["desired"]=="stopped"
assert snapshot["outputStatuses"][0]["actual"]=="stopped"
recoveries={item["recoveryStatus"] for item in recordings}
assert "recovered" in recoveries, recoveries
assert "partial" in recoveries, recoveries
assert sum(item["validation"]=="decoded" for item in recordings)>=2, recordings
assert all("abrupt_shutdown" in item["gaps"] for item in recordings), recordings
for item in recordings:
    path=pathlib.Path(item["path"]); digest=hashlib.sha256(path.read_bytes()).hexdigest()
    assert expected[str(path)]==digest==item["sha256"], item
    assert item["engineRunId"]==initial["engineRunId"], item
    assert item["mappingStatus"] in {"mapped","unmapped"}
    if item["recoveryStatus"]=="partial": assert item["validation"]=="partial"
inventory=exported["pluginResult"]["mediaInventory"]
assert len(inventory)==len(recordings), (len(inventory),len(recordings))
assert any(gap["kind"]=="recording" and gap["detail"]=="abrupt_shutdown" for gap in exported["pluginResult"]["gaps"])
closed_before=sum(item["decoded"] for item in json.load(open(sys.argv[5])))
assert closed_before>=2, closed_before
print(json.dumps({"closedBeforeKill":closed_before,"fragments":len(recordings),"recoveries":sorted(recoveries),"outputsStopped":True,"hashesPreserved":True,"exportInventory":len(inventory)},sort_keys=True))
PY

find "$OUTPUT_DIR/state/recordings" -name 'segment-*.mkv' -type f -print0 \
  | sort -z | xargs -0 sha256sum >"$OUTPUT_DIR/after-restart.sha256"
cmp "$OUTPUT_DIR/before.sha256" "$OUTPUT_DIR/after-restart.sha256"

kill -TERM "$app_pid"
wait "$app_pid"
app_pid=
printf '%s\n' "output_crash_recovery=passed" "artifacts=$OUTPUT_DIR"
