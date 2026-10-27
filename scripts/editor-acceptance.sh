#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
source "$ROOT/scripts/env.sh"
cd "$ROOT"

cargo test --locked -p live-production --test extensions --test editor_extensions
