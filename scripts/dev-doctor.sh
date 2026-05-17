#!/bin/bash
# Checks the dev environment for all required tools, services, and fixture files.
# Reports PASS/FAIL for each check with a remedy for every failure.
#
# Run this script when setting up a new environment or diagnosing CI-like failures:
#   bash scripts/dev-doctor.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# --- Colours ---------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

# --- Tracking --------------------------------------------------------------
PASSES=()
FAILS=()

pass() {
  PASSES+=("$1")
  printf "  ${GREEN}[PASS]${NC} %s\n" "$1"
}

fail() {
  FAILS+=("$1")
  printf "  ${RED}[FAIL]${NC} %s\n" "$1"
  printf "         → %s\n" "$2"
}

# --- Tools -----------------------------------------------------------------
echo ""
echo "==> Tools"
echo ""

# Rust
REQUIRED_RUST="1.95.0"
if command -v rustc &>/dev/null; then
  RUST_VER=$(rustc --version | awk '{print $2}')
  if [[ "$RUST_VER" == "$REQUIRED_RUST" ]]; then
    pass "rustc $RUST_VER (need $REQUIRED_RUST)"
  else
    fail "rustc $RUST_VER (need $REQUIRED_RUST)" \
      "Install Rust $REQUIRED_RUST via rustup: https://rustup.rs/"
  fi
else
  fail "rustc not found" \
    "Install Rust $REQUIRED_RUST via rustup: https://rustup.rs/"
fi

# rustfmt
REQUIRED_RUSTFMT="1.9.0-stable"
if command -v rustfmt &>/dev/null; then
  RUSTFMT_VER=$(rustfmt --version | awk '{print $2}')
  if [[ "$RUSTFMT_VER" == "$REQUIRED_RUSTFMT" ]]; then
    pass "rustfmt $RUSTFMT_VER (need $REQUIRED_RUSTFMT)"
  else
    fail "rustfmt $RUSTFMT_VER (need $REQUIRED_RUSTFMT)" \
      "Run: rustup component add rustfmt"
  fi
else
  fail "rustfmt not found" \
    "Run: rustup component add rustfmt"
fi

# clippy
REQUIRED_CLIPPY="0.1.$(echo "$REQUIRED_RUST" | cut -d. -f2)"
if cargo clippy --version &>/dev/null 2>&1; then
  CLIPPY_VER=$(cargo clippy --version | awk '{print $2}')
  if [[ "$CLIPPY_VER" == "$REQUIRED_CLIPPY" ]]; then
    pass "clippy $CLIPPY_VER (need $REQUIRED_CLIPPY)"
  else
    fail "clippy $CLIPPY_VER (need $REQUIRED_CLIPPY)" \
      "Run: rustup component add clippy"
  fi
else
  fail "clippy not found" \
    "Run: rustup component add clippy"
fi

# protoc
REQUIRED_PROTOC_MAJOR="34"
if command -v protoc &>/dev/null; then
  PROTOC_VER=$(protoc --version | awk '{print $2}')
  PROTOC_MAJOR="${PROTOC_VER%%.*}"
  if [[ "${PROTOC_MAJOR}" -eq "${REQUIRED_PROTOC_MAJOR}" ]]; then
    pass "protoc $PROTOC_VER (need major $REQUIRED_PROTOC_MAJOR)"
  else
    fail "protoc $PROTOC_VER (need major $REQUIRED_PROTOC_MAJOR)" \
      "Rebuild the devcontainer, or download v${REQUIRED_PROTOC_MAJOR} from https://github.com/protocolbuffers/protobuf/releases"
  fi
else
  fail "protoc not found" \
    "Rebuild the devcontainer, or download v${REQUIRED_PROTOC_MAJOR} from https://github.com/protocolbuffers/protobuf/releases"
fi

# Docker
REQUIRED_DOCKER_MAJOR="29"
if command -v docker &>/dev/null; then
  DOCKER_VER=$(docker --version | awk '{print $3}' | tr -d ',')
  DOCKER_MAJOR="${DOCKER_VER%%.*}"
  if [[ "${DOCKER_MAJOR}" -eq "${REQUIRED_DOCKER_MAJOR}" ]]; then
    pass "docker $DOCKER_VER (need major $REQUIRED_DOCKER_MAJOR)"
  else
    fail "docker $DOCKER_VER (need major $REQUIRED_DOCKER_MAJOR)" \
      "Install Docker: https://docs.docker.com/get-docker/"
  fi
  if docker info &>/dev/null 2>&1; then
    pass "docker daemon running"
  else
    fail "docker daemon not running" \
      "Start Docker Desktop, or: sudo systemctl start docker"
  fi
else
  fail "docker not found" \
    "Install Docker: https://docs.docker.com/get-docker/"
fi

# Claude Code (optional; when present, intent is also required)
REQUIRED_CLAUDE_MAJOR="2"
CLAUDE_PRESENT=false
if command -v claude &>/dev/null; then
  CLAUDE_VER=$(claude --version 2>/dev/null | awk '{print $1}')
  CLAUDE_MAJOR="${CLAUDE_VER%%.*}"
  if [[ "${CLAUDE_MAJOR}" -eq "${REQUIRED_CLAUDE_MAJOR}" ]]; then
    pass "claude $CLAUDE_VER (need major $REQUIRED_CLAUDE_MAJOR)"
  else
    fail "claude $CLAUDE_VER (need major $REQUIRED_CLAUDE_MAJOR)" \
      "Install Claude Code: https://claude.ai/code"
  fi
  CLAUDE_PRESENT=true
fi

# intent (required when Claude Code is present)
if [[ "${CLAUDE_PRESENT}" == true ]]; then
  REQUIRED_INTENT="2.11.6"
  if command -v intent &>/dev/null; then
    INTENT_VER=$(intent --version 2>/dev/null | awk '{print $3}')
    if [[ "$INTENT_VER" == "$REQUIRED_INTENT" ]]; then
      pass "intent $INTENT_VER (need $REQUIRED_INTENT)"
    else
      fail "intent $INTENT_VER (need $REQUIRED_INTENT)" \
        "Install intent $REQUIRED_INTENT: https://github.com/matthewsinclair/intent"
    fi
  else
    fail "intent not found (required when Claude Code is present)" \
      "Install intent $REQUIRED_INTENT: https://github.com/matthewsinclair/intent"
  fi
fi

# docker compose
REQUIRED_COMPOSE_MAJOR="5"
if docker compose version &>/dev/null 2>&1; then
  COMPOSE_VER=$(docker compose version 2>/dev/null | awk '{print $NF}')
  COMPOSE_MAJOR="${COMPOSE_VER#v}"
  COMPOSE_MAJOR="${COMPOSE_MAJOR%%.*}"
  if [[ "${COMPOSE_MAJOR}" -eq "${REQUIRED_COMPOSE_MAJOR}" ]]; then
    pass "docker compose $COMPOSE_VER (need major $REQUIRED_COMPOSE_MAJOR)"
  else
    fail "docker compose $COMPOSE_VER (need major $REQUIRED_COMPOSE_MAJOR)" \
      "Install Docker Compose v2: https://docs.docker.com/compose/install/"
  fi
else
  fail "docker compose not available" \
    "Install Docker Compose v2: https://docs.docker.com/compose/install/"
fi

# k3d (optional; when present, kubectl and helm are also required)
REQUIRED_K3D_MAJOR="5"
K3D_PRESENT=false
if command -v k3d &>/dev/null; then
  K3D_VER=$(k3d version 2>/dev/null | head -1 | awk '{print $3}' | tr -d 'v')
  K3D_MAJOR="${K3D_VER%%.*}"
  if [[ "${K3D_MAJOR}" -eq "${REQUIRED_K3D_MAJOR}" ]]; then
    pass "k3d $K3D_VER (need major $REQUIRED_K3D_MAJOR)"
  else
    fail "k3d $K3D_VER (need major $REQUIRED_K3D_MAJOR)" \
      "Install k3d: https://k3d.io/stable/#installation"
  fi
  K3D_PRESENT=true
fi

# kubectl and helm (required when k3d is present)
if [[ "${K3D_PRESENT}" == true ]]; then
  REQUIRED_KUBECTL_MAJOR="1"
  if command -v kubectl &>/dev/null; then
    KUBECTL_VER=$(kubectl version --client --output=json 2>/dev/null | jq -r '.clientVersion.gitVersion' | tr -d 'v')
    KUBECTL_MAJOR="${KUBECTL_VER%%.*}"
    if [[ "${KUBECTL_MAJOR}" -eq "${REQUIRED_KUBECTL_MAJOR}" ]]; then
      pass "kubectl $KUBECTL_VER (need major $REQUIRED_KUBECTL_MAJOR)"
    else
      fail "kubectl $KUBECTL_VER (need major $REQUIRED_KUBECTL_MAJOR)" \
        "Install kubectl: https://kubernetes.io/docs/tasks/tools/"
    fi
  else
    fail "kubectl not found (required when k3d is present)" \
      "Install kubectl: https://kubernetes.io/docs/tasks/tools/"
  fi

  REQUIRED_HELM_MAJOR="4"
  if command -v helm &>/dev/null; then
    HELM_VER=$(helm version --short 2>/dev/null | cut -d'+' -f1 | tr -d 'v')
    HELM_MAJOR="${HELM_VER%%.*}"
    if [[ "${HELM_MAJOR}" -eq "${REQUIRED_HELM_MAJOR}" ]]; then
      pass "helm $HELM_VER (need major $REQUIRED_HELM_MAJOR)"
    else
      fail "helm $HELM_VER (need major $REQUIRED_HELM_MAJOR)" \
        "Install Helm: https://helm.sh/docs/intro/install/"
    fi
  else
    fail "helm not found (required when k3d is present)" \
      "Install Helm: https://helm.sh/docs/intro/install/"
  fi
fi

# openssl (required by scripts/gen-env.sh)
REQUIRED_OPENSSL_MAJOR="3"
if command -v openssl &>/dev/null; then
  OPENSSL_VER=$(openssl version | awk '{print $2}')
  OPENSSL_MAJOR="${OPENSSL_VER%%.*}"
  if [[ "${OPENSSL_MAJOR}" -eq "${REQUIRED_OPENSSL_MAJOR}" ]]; then
    pass "openssl $OPENSSL_VER (need major $REQUIRED_OPENSSL_MAJOR)"
  else
    fail "openssl $OPENSSL_VER (need major $REQUIRED_OPENSSL_MAJOR)" \
      "Debian/Ubuntu: apt-get install openssl   macOS: brew install openssl"
  fi
else
  fail "openssl not found" \
    "Debian/Ubuntu: apt-get install openssl   macOS: brew install openssl"
fi

# curl (required by service health checks and scripts)
REQUIRED_CURL_MAJOR="7"
if command -v curl &>/dev/null; then
  CURL_VER=$(curl --version 2>/dev/null | head -1 | awk '{print $2}')
  CURL_MAJOR="${CURL_VER%%.*}"
  if [[ "${CURL_MAJOR}" -eq "${REQUIRED_CURL_MAJOR}" ]]; then
    pass "curl $CURL_VER (need major $REQUIRED_CURL_MAJOR)"
  else
    fail "curl $CURL_VER (need major $REQUIRED_CURL_MAJOR)" \
      "Debian/Ubuntu: apt-get install curl   macOS: brew install curl"
  fi
else
  fail "curl not found" \
    "Debian/Ubuntu: apt-get install curl   macOS: brew install curl"
fi

# --- Environment & fixtures ------------------------------------------------
echo ""
echo "==> Environment & fixtures"
echo ""

ENV_FILE="${WORKSPACE_DIR}/.env"

if [[ -f "${ENV_FILE}" ]]; then
  pass ".env present"
  # Source it so DATABASE_URL and other vars are available for later checks.
  set -o allexport
  # shellcheck source=/dev/null
  source "${ENV_FILE}"
  set +o allexport
else
  fail ".env not found" \
    "Run: bash scripts/gen-env.sh"
fi

if [[ -n "${DATABASE_URL:-}" ]]; then
  pass "DATABASE_URL set"
else
  fail "DATABASE_URL not set" \
    "Run: bash scripts/gen-env.sh  (writes DATABASE_URL to .env)"
fi

# JWT and TLS key material required by the test suite.
JWT_DIR="${WORKSPACE_DIR}/projects/rust/server/tests/jwt"
TLS_DIR="${WORKSPACE_DIR}/projects/rust/server/tests/certs"
ALL_KEYS=true
for f in \
  "${JWT_DIR}/signing_private_key.pem" \
  "${JWT_DIR}/signing_public_key.pem" \
  "${JWT_DIR}/bad_signing_private_key.pem" \
  "${JWT_DIR}/bad_signing_public_key.pem" \
  "${TLS_DIR}/ca.key" \
  "${TLS_DIR}/ca.crt" \
  "${TLS_DIR}/server.key" \
  "${TLS_DIR}/server.crt"; do
  [[ -f "$f" ]] || { ALL_KEYS=false; break; }
done
if [[ "${ALL_KEYS}" == true ]]; then
  pass "key material present (TLS certs + JWT signing keys)"
else
  fail "key material missing" \
    "Run: bash scripts/gen-keys-and-certs.sh"
fi

# --- Services --------------------------------------------------------------
echo ""
echo "==> Services"
echo ""

# PostgreSQL — use pg_isready if available, otherwise probe the TCP port.
POSTGRES_READY=false
if command -v pg_isready &>/dev/null; then
  pg_isready -h localhost -p 5432 -U postgres -q 2>/dev/null && POSTGRES_READY=true
else
  bash -c 'echo > /dev/tcp/localhost/5432' 2>/dev/null && POSTGRES_READY=true
fi
if [[ "${POSTGRES_READY}" == true ]]; then
  pass "PostgreSQL accepting connections (localhost:5432)"
else
  fail "PostgreSQL not accepting connections (localhost:5432)" \
    "Run: docker compose -f projects/compose/docker-compose.yml --env-file .env up -d"
fi

# Hydra admin API
HYDRA_ADMIN="${HYDRA_ADMIN_URL:-http://localhost:4445}"
if curl -sf --max-time 5 "${HYDRA_ADMIN}/health/ready" &>/dev/null; then
  HYDRA_VER=$(curl -sf --max-time 5 "${HYDRA_ADMIN}/version" 2>/dev/null | grep -o '"version":"[^"]*"' | cut -d'"' -f4 || echo "unknown")
  pass "Hydra admin API healthy — ${HYDRA_ADMIN} (${HYDRA_VER})"
else
  fail "Hydra admin API not reachable (${HYDRA_ADMIN})" \
    "Run: docker compose -f projects/compose/docker-compose.yml --env-file .env up -d"
fi

# --- Summary ---------------------------------------------------------------
echo ""
echo "============================================"
TOTAL=$(( ${#PASSES[@]} + ${#FAILS[@]} ))
printf "  %d/%d checks passed\n" "${#PASSES[@]}" "${TOTAL}"
echo "============================================"
echo ""

if [[ ${#FAILS[@]} -gt 0 ]]; then
  printf "${RED}%d check(s) failed — fix the issues above and re-run this script.${NC}\n\n" "${#FAILS[@]}"
  exit 1
else
  printf "${GREEN}All checks passed — environment is ready.${NC}\n\n"
fi
