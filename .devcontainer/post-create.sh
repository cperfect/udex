#!/bin/bash
RUST_VERSION="${RUST_VERISON:-stable}"
set -e
rustup upgrade ${RUST_VERSION}
rustup default ${RUST_VERSION}
rustup component add clippy
rustup component add rustfmt
rustup show

npm install -g @anthropic-ai/claude-code