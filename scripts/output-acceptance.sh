#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUTPUT_DIR=${1:-$PROJECT_ROOT/var/output-acceptance}

mkdir -p "$OUTPUT_DIR"
sh "$PROJECT_ROOT/scripts/output-recording-gate.sh" "$OUTPUT_DIR/recording"
sh "$PROJECT_ROOT/scripts/output-gates.sh" "$OUTPUT_DIR/network"
sh "$PROJECT_ROOT/scripts/output-shutdown-gate.sh" "$OUTPUT_DIR/shutdown"
sh "$PROJECT_ROOT/scripts/output-browser-gate.sh" "$OUTPUT_DIR/browser"
printf '%s\n' "output_acceptance=passed" "artifacts=$OUTPUT_DIR"
