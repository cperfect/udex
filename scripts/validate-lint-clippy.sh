#!/bin/bash
# Mirrors the "Clippy" step in 01-Validation.yml (lint job).
# Run from any directory.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cd "${SCRIPT_DIR}/../projects/rust"
cargo clippy --all-targets -- -D warnings
