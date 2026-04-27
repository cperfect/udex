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

# install hydra CLI (pinned release, checksum-verified)
HYDRA_VERSION="v26.2.0"
HYDRA_TARBALL="hydra_26.2.0-linux_64bit.tar.gz"
HYDRA_RELEASE_URL="https://github.com/ory/hydra/releases/download/${HYDRA_VERSION}"

hydra_tmp="$(mktemp -d)"
trap 'rm -rf "${hydra_tmp}"' EXIT

curl -fsSL "${HYDRA_RELEASE_URL}/${HYDRA_TARBALL}" -o "${hydra_tmp}/${HYDRA_TARBALL}"
curl -fsSL "${HYDRA_RELEASE_URL}/checksums.txt"    -o "${hydra_tmp}/checksums.txt"

# Verify SHA-256 against the publisher's pinned release checksums
(cd "${hydra_tmp}" && grep "${HYDRA_TARBALL}" checksums.txt | sha256sum --check --strict --quiet)

tar -xzf "${hydra_tmp}/${HYDRA_TARBALL}" -C "${hydra_tmp}" hydra
sudo mv "${hydra_tmp}/hydra" /usr/local/bin/
hydra help
