#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
PLUGIN_DIR=
EVIDENCE_DIR=
SUPERVISOR="$ROOT/target/release/live-production-supervisor"
SERVER="$ROOT/target/release/live-production"
OBS=obs
DEADLINE=300
DRY_RUN=false
previous=
for argument in "$@"; do
  case "$previous" in
    plugin) PLUGIN_DIR=$argument ;;
    evidence) EVIDENCE_DIR=$argument ;;
    supervisor) SUPERVISOR=$argument ;;
    server) SERVER=$argument ;;
    obs) OBS=$argument ;;
    deadline) DEADLINE=$argument ;;
  esac
  case "$argument" in
    --plugin-dir) previous=plugin ;;
    --evidence-dir) previous=evidence ;;
    --supervisor-binary) previous=supervisor ;;
    --server-binary) previous=server ;;
    --obs-binary) previous=obs ;;
    --deadline-seconds) previous=deadline ;;
    --dry-run) DRY_RUN=true; previous= ;;
    --*) if [ "$previous" != plugin ] && [ "$previous" != evidence ] && [ "$previous" != supervisor ] && [ "$previous" != server ] && [ "$previous" != obs ] && [ "$previous" != deadline ]; then previous=other; fi ;;
    *) previous= ;;
  esac
done
if [ -z "$PLUGIN_DIR" ] || [ -z "$EVIDENCE_DIR" ]; then
  printf '%s\n' "Usage: scripts/target/rehearsal.sh --plugin-dir STAGE --evidence-dir PRIVATE [options]" >&2
  exit 2
fi
umask 077
mkdir -m 700 "$EVIDENCE_DIR"
mkdir -m 700 "$EVIDENCE_DIR/private"
if [ "$DRY_RUN" = true ]; then
  exec node "$ROOT/scripts/target/rehearsal.mjs" "$@"
fi
PREFLIGHT="$EVIDENCE_DIR/private/preflight.json"
"$ROOT/scripts/target/preflight.sh" --plugin-dir "$PLUGIN_DIR" --report "$PREFLIGHT" \
  --server-binary "$SERVER" --supervisor-binary "$SUPERVISOR" --obs-binary "$OBS"
test -x "$SUPERVISOR"
exec "$SUPERVISOR" --timeout-ms "$((DEADLINE * 1000 + 60000))" --grace-ms 15000 -- \
  node "$ROOT/scripts/target/rehearsal.mjs" "$@" --preflight-report "$PREFLIGHT"
