#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$PROJECT_ROOT/scripts/smoke-supervisor.sh"
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/output-acceptance}

case "$OUTPUT_DIR" in
  "$PROJECT_ROOT"/var/*|var/*) ;;
  *) printf '%s\n' "output evidence must stay under $PROJECT_ROOT/var" >&2; exit 1 ;;
esac
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
sh "$PROJECT_ROOT/scripts/output-recording-gate.sh" "$OUTPUT_DIR/recording"
sh "$PROJECT_ROOT/scripts/output-gates.sh" "$OUTPUT_DIR/network"
sh "$PROJECT_ROOT/scripts/output-shutdown-gate.sh" "$OUTPUT_DIR/shutdown"
sh "$PROJECT_ROOT/scripts/output-browser-gate.sh" "$OUTPUT_DIR/browser"
printf '%s\n' "output_acceptance=passed" "artifacts=$OUTPUT_DIR"
