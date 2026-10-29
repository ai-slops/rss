#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
SUPERVISOR="$ROOT/target/release/live-production-supervisor"
DEADLINE=180
previous=
for argument in "$@"; do
  case "$previous" in
    supervisor) SUPERVISOR=$argument ;;
    deadline) DEADLINE=$argument ;;
  esac
  case "$argument" in
    --supervisor-binary) previous=supervisor ;;
    --deadline-seconds) previous=deadline ;;
    *) previous= ;;
  esac
done
test -x "$SUPERVISOR"
exec "$SUPERVISOR" --timeout-ms "$((DEADLINE * 1000 + 30000))" --grace-ms 10000 -- \
  node "$ROOT/scripts/target/recovery-native.mjs" "$@"
