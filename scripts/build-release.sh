#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$project_dir/target}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
cd "$project_dir"
npm run build
