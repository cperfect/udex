#!/bin/bash
RUST_VERSION="${RUST_VERISON:-stable}"
set -e
rustup upgrade ${RUST_VERSION}
rustup default ${RUST_VERSION}
rustup component add clippy
rustup component add rustfmt
rustup show

# intent setup
# Verify installation
intent --version
cd /workspace
# Install Claude Code subagent 
intent claude subagents install intent
# Install CLaude Code skills
intent claude skills install in-essentials
