#!/bin/bash
RUST_VERSION="${RUST_VERSION:-stable}"
set -e
rustup upgrade "${RUST_VERSION}"
rustup default "${RUST_VERSION}"
rustup component add clippy
rustup component add rustfmt
rustup show

# install claude code
# npm install -g @anthropic-ai/claude-code
# curl -fsSL https://claude.ai/install.sh | bash
# verify claude
claude --version

# intent setup
# Verify installation
intent --version
cd /workspace

# moved to last as it seems flakey?
# # Install Claude Code subagent 
# intent claude subagents install intent
# # Install CLaude Code skills
# intent claude skills install in-essentials

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

# ── Developer secret & key setup ──────────────────────────────────────────────
# The workspace volume persists across container rebuilds, so these only run
# when the output files are absent (fresh clone or after manual deletion).

cd /workspace

if [[ ! -f .env ]]; then
  echo "No .env found — generating dev secrets..."
  bash scripts/gen-env.sh --force
else
  echo ".env already exists — skipping secret generation."
fi

# need to link the .env into decontainer
# so docker compose can pick it up
if [ ! -L ".devcontainer/.env" ]; then
  ln -s .env .devcontainer/.env
fi

if [[ ! -f projects/rust/server/tests/jwt/signing_private_key.pem ]]; then
  echo "Key material absent — generating certs and keys..."
  bash scripts/gen-keys-and-certs.sh
else
  echo "Key material already exists — skipping key generation."
fi

# see comment above
# Install Claude Code subagent 
intent claude subagents install intent
# Install CLaude Code skills
intent claude skills install in-essentials
