#!/bin/bash
# Generates the dev env file with fresh secret values.
# Public/config items use well-known dev defaults.
#
# The real file lives at the workspace root .env and .devcontainer/.env is a
# relative symlink to it. This keeps a single source of truth (Highlander):
# tools that look for ./.env and tools that mount .devcontainer/.env both
# resolve to the same file.
#
# Usage:
#   scripts/gen-env.sh                  # prompts if an env file already exists
#   scripts/gen-env.sh --force          # overwrites without prompting
#   scripts/gen-env.sh --force --rotate-live  # also rotate against a live Postgres
#
# Run once when first cloning the repo. Re-run with --force to rotate secrets.
# Re-running is idempotent: it converges to the same file + symlink layout
# regardless of the starting state. The devcontainer post-create script calls
# this automatically.
#
# Rotation safety: Postgres only applies POSTGRES_PASSWORD on first init, so a
# persisted data volume keeps its old password when you rotate .env. Because
# pg_hba trusts loopback but requires scram elsewhere, host tools keep working
# while scram clients (e.g. k8s pods) fail to authenticate far from the cause.
# This script therefore refuses to rotate an existing .env while a compose
# Postgres is running, unless you pass --rotate-live. To rotate cleanly, tear
# the stack's volumes down first (see docs/SECRETS.md).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEVCONTAINER_DIR="${WORKSPACE_DIR}/.devcontainer"
ENV_FILE="${WORKSPACE_DIR}/.env"      # the real file (mode 600)
ENV_LINK="${DEVCONTAINER_DIR}/.env"   # relative symlink -> ../.env

# Detect a running compose Postgres. The compose-service label is independent of
# the project name, so this works regardless of where the stack was brought up.
# Returns non-zero (not detected) when docker is unavailable, so first-clone and
# CI flows without a live stack are unaffected.
running_postgres_detected() {
  command -v docker &>/dev/null || return 1
  local ids
  ids="$(docker ps -q --filter "label=com.docker.compose.service=postgres" 2>/dev/null)" || return 1
  [[ -n "${ids}" ]]
}

FORCE=false
ROTATE_LIVE=false
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=true ;;
    --rotate-live) ROTATE_LIVE=true ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

# Does an env file already exist anywhere? -e follows symlinks, so this catches a
# stale layout (root .env itself a symlink) as well as the normal one. Anything
# beyond this point that hinges on "are we rotating vs first-generating" reads
# this flag.
env_exists=false
if [[ -e "${ENV_FILE}" || -e "${ENV_LINK}" ]]; then
  env_exists=true
fi

# Rotation safety guard: refuse to rotate an existing .env while a compose
# Postgres is live, because the DB keeps its init-time password and only scram
# clients (k8s) would notice the drift. Fires regardless of --force; --rotate-live
# is the explicit opt-out. First generation (no existing .env) is always allowed.
if [[ "${env_exists}" == true && "${ROTATE_LIVE}" == false ]] && running_postgres_detected; then
  compose_down="docker compose -f projects/compose/docker-compose.yml --env-file .env down -v"
  {
    echo "ERROR: a running compose Postgres was detected."
    echo "Rotating .env will NOT change the already-initialized database password"
    echo "and will break scram clients (e.g. k8s pods) while loopback trust auth"
    echo "hides the drift."
    echo ""
    echo "  To rotate for real (destroys local DB data):"
    echo "    ${compose_down} && scripts/gen-env.sh --force"
    echo "  To override this guard anyway:"
    echo "    scripts/gen-env.sh --force --rotate-live"
    echo ""
    echo "Aborted. Existing .env unchanged."
  } >&2
  exit 1
fi

# Prompt if an env file already exists and we're not forcing.
if [[ "${env_exists}" == true && "${FORCE}" == false ]]; then
  read -r -p ".env already exists. Overwrite and rotate all secrets? [y/N] " reply
  if [[ ! "${reply}" =~ ^[Yy]$ ]]; then
    echo "Aborted. Existing .env unchanged."
    exit 0
  fi
fi

echo "Generating secrets..."

# Generate secret values. The values are expanded inside an unquoted heredoc
# below, so every generator here must stay within an alphabet that cannot be
# reinterpreted by the shell — no $, `, \, ', ", or whitespace — and must also
# survive docker compose's .env parser, which rules out # as well.
#
# `openssl rand -hex` produces [0-9a-f] only and satisfies that trivially.
POSTGRES_PASSWORD_SECRET_VAL=$(openssl rand -hex 24)
HYDRA_DB_PASSWORD_SECRET_VAL=$(openssl rand -hex 24)
HYDRA_SECRETS_SYSTEM_SECRET_VAL=$(openssl rand -hex 32)

# OpenObserve (ST0028) will not accept a hex password. It enforces 8-128 chars
# with at least one lowercase, one uppercase, one digit and one special
# character, and PANICS on first boot when the rule is not met — so the fixture
# fails to start rather than degrading quietly.
#
# The fixed `Ux1-` prefix supplies the four required character classes; the
# entropy is entirely in the 48 hex characters that follow. That keeps the whole
# value inside [A-Za-z0-9-], which is still safe for the unquoted heredoc and for
# compose. Do not "improve" this into a random symbol soup without re-reading the
# heredoc note above — $ or # in this value breaks the fixture in ways that are
# tedious to diagnose.
# Generated in two steps on purpose: the entropy is captured on its own so the
# emptiness guard below can actually see it. Composing the prefix inline would
# yield a non-empty "Ux1-" when openssl fails, sailing straight past the check.
OPENOBSERVE_ROOT_EMAIL_VAL="root@udex.local"
OPENOBSERVE_PASSWORD_ENTROPY=$(openssl rand -hex 24)
OPENOBSERVE_ROOT_PASSWORD_SECRET_VAL="Ux1-${OPENOBSERVE_PASSWORD_ENTROPY}"

# Pre-encode the Basic credential the OTel Collector sends to OpenObserve, and
# the tests use to query it. Encoding once here keeps the collector config
# declarative — compose cannot base64 inline, and the fixture's no-bind-mount
# rule makes a shell wrapper in the collector container awkward. base64's
# alphabet (A-Za-z0-9+/=) is heredoc-safe. -w0 keeps it on one line; GNU coreutils
# only, which is what the devcontainer and CI both run.
OPENOBSERVE_BASIC_AUTH_SECRET_VAL=$(printf '%s' \
  "${OPENOBSERVE_ROOT_EMAIL_VAL}:${OPENOBSERVE_ROOT_PASSWORD_SECRET_VAL}" | base64 -w0)

# Guard: if openssl is missing or fails, command substitution returns an empty
# string. set -e won't catch that (it only catches non-zero exits at the
# statement level). Fail fast here rather than writing a .env full of blanks.
if [[ -z "${POSTGRES_PASSWORD_SECRET_VAL}" || \
      -z "${HYDRA_DB_PASSWORD_SECRET_VAL}" || \
      -z "${HYDRA_SECRETS_SYSTEM_SECRET_VAL}" || \
      -z "${OPENOBSERVE_PASSWORD_ENTROPY}" || \
      -z "${OPENOBSERVE_BASIC_AUTH_SECRET_VAL}" ]]; then
  echo "ERROR: secret generation produced an empty value — is openssl installed?" >&2
  exit 1
fi

# Hydra URLs: callers may export these before invoking this script to override
# the defaults. The devcontainer post-create does this so that the .env it
# generates uses the Docker service name rather than localhost.
HYDRA_PUBLIC_URL="${HYDRA_PUBLIC_URL:-http://localhost:4444}"
HYDRA_ADMIN_URL="${HYDRA_ADMIN_URL:-http://localhost:4445}"

# Ensure the devcontainer dir exists (for the link) and clear any stale entry at
# ENV_FILE. The clear matters: if root .env is currently a symlink (e.g. it was
# flipped to point at .devcontainer/.env), an unguarded `cat >` would write
# *through* the link instead of replacing it with the real file. rm -f is a
# no-op when absent.
mkdir -p "${DEVCONTAINER_DIR}"
rm -f "${ENV_FILE}"

cat > "${ENV_FILE}" <<EOF
# ============================================================
# Udex development environment — DO NOT COMMIT
# Generated by scripts/gen-env.sh on $(date -u +"%Y-%m-%dT%H:%M:%SZ")
# Re-run with --force to rotate all secrets.
# ============================================================

# ------------------------------------------------------------
# PostgreSQL
# ------------------------------------------------------------
# Public config
POSTGRES_USER=postgres
POSTGRES_DB=postgres

# Secrets — injected into docker-compose and the devcontainer
POSTGRES_PASSWORD_SECRET=${POSTGRES_PASSWORD_SECRET_VAL}

# Derived — full connection URL for cargo test and the CLI
DATABASE_URL=postgres://postgres:${POSTGRES_PASSWORD_SECRET_VAL}@localhost:5432/postgres

# ------------------------------------------------------------
# Hydra (OAuth2 server)
# ------------------------------------------------------------
# Public config — override before calling gen-env.sh to change these values.
HYDRA_PUBLIC_URL=${HYDRA_PUBLIC_URL}
HYDRA_ADMIN_URL=${HYDRA_ADMIN_URL}
HYDRA_ISSUER=http://localhost:4444/

# Secrets
HYDRA_DB_PASSWORD_SECRET=${HYDRA_DB_PASSWORD_SECRET_VAL}
HYDRA_SECRETS_SYSTEM_SECRET=${HYDRA_SECRETS_SYSTEM_SECRET_VAL}

# ------------------------------------------------------------
# Observability (ST0027 / ST0028)
# ------------------------------------------------------------
# The obs fixture is part of the base projects/compose stack. The OTLP hop from
# the app to the collector stays keyless; OpenObserve is the one component that
# requires credentials, on both ingest and query.
#
# Auth terminates at the collector: it holds the encoded credential below so the
# udex application keeps emitting anonymous OTLP and never learns the backend
# wants one. The integration tests use the same values to query.
OPENOBSERVE_ROOT_EMAIL=${OPENOBSERVE_ROOT_EMAIL_VAL}
OPENOBSERVE_ROOT_PASSWORD_SECRET=${OPENOBSERVE_ROOT_PASSWORD_SECRET_VAL}

# Derived — base64(email:password), consumed as the collector's Basic auth
# header. Rotating the password above without regenerating this will authenticate
# nothing; always rotate by re-running this script rather than hand-editing.
OPENOBSERVE_BASIC_AUTH_SECRET=${OPENOBSERVE_BASIC_AUTH_SECRET_VAL}
EOF

chmod 600 "${ENV_FILE}"

# Expose the real root file inside .devcontainer via a relative symlink. -f
# replaces whatever is there (an old real .env or a prior symlink); -n avoids
# following an existing symlinked dir. The target is relative so the link is
# path-portable.
ln -sfn "../.env" "${ENV_LINK}"

echo ".env written to ${ENV_FILE}"
echo "  and symlinked at ${ENV_LINK} -> ../.env"
echo ""
echo "Next step: run scripts/gen-keys-and-certs.sh to generate TLS and JWT key material."
