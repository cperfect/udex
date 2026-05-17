#!/bin/bash
# Mirrors the "Build" and "Test" steps in 01-Validation.yml (Build & Test job).
# Run from any directory. Requires DATABASE_URL and Hydra env vars to be set
# (the devcontainer provides these; in CI they are set by the workflow env block).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cd "${SCRIPT_DIR}/../projects/rust"
cargo build --all-targets
cargo test
