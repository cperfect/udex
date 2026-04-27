#!/bin/bash
RUST_VERSION="${RUST_VERSION:-stable}"
set -e
rustup upgrade "${RUST_VERSION}"
rustup default "${RUST_VERSION}"
rustup component add clippy
rustup component add rustfmt
rustup show

# install claude code
npm install -g @anthropic-ai/claude-code
claude --version

# intent setup
# Verify installation
intent --version
cd /workspace
# Install Claude Code subagent 
intent claude subagents install intent
# Install CLaude Code skills
intent claude skills install in-essentials

# intall hydra so we can use the cli
bash <(curl https://raw.githubusercontent.com/ory/meta/master/install.sh) -d -b . hydra v26.2.0
sudo mv ./hydra /usr/local/bin/
hydra help
