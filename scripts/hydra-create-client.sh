#!/usr/bin/env bash
# Create (or replace) an Ory Hydra OAuth2 client suitable for manual CLI testing.
#
# Usage:
#   scripts/hydra-create-client.sh --client-id <id> --scope <scope> [--scope <scope> ...] [options]
#
# Options:
#   --client-id ID       OAuth2 client ID (required)
#   --scope SCOPE        Scope to grant; may be repeated (required, at least one)
#   --admin-url URL      Hydra admin URL (default: $HYDRA_ADMIN_URL or http://hydra:4445)
#   --token-url URL      OAuth2 token URL printed in output (default: $HYDRA_PUBLIC_URL/oauth2/token)
#   --audience AUD       Token audience (default: $HYDRA_ISSUER or http://localhost:4444/)
#
# The client secret is generated randomly. On success the script prints the
# environment variables needed to run the udex CLI.
#
# Example — grant full access to one index:
#   scripts/hydra-create-client.sh \
#     --client-id dev-cli \
#     --scope udex:index:v1:list \
#     --scope udex:index:v1:my-index:read \
#     --scope udex:entry:v1:my-index:create \
#     --scope udex:entry:v1:my-index:read \
#     --scope udex:entry:v1:my-index:write \
#     --scope udex:entry:v1:my-index:delete

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────

ADMIN_URL="${HYDRA_ADMIN_URL:-http://hydra:4445}"
TOKEN_URL="${HYDRA_PUBLIC_URL:-http://hydra:4444}/oauth2/token"
AUDIENCE="${HYDRA_ISSUER:-http://localhost:4444/}"
CLIENT_ID=""
SCOPES=()

# ── Argument parsing ──────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --client-id)  CLIENT_ID="$2";  shift 2 ;;
        --scope)      SCOPES+=("$2"); shift 2 ;;
        --admin-url)  ADMIN_URL="$2";  shift 2 ;;
        --token-url)  TOKEN_URL="$2";  shift 2 ;;
        --audience)   AUDIENCE="$2";   shift 2 ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

if [[ -z "$CLIENT_ID" || ${#SCOPES[@]} -eq 0 ]]; then
    echo "Usage: $0 --client-id <id> --scope <scope> [--scope <scope> ...] [--admin-url URL] [--token-url URL] [--audience AUD]" >&2
    exit 1
fi

# ── Helpers ───────────────────────────────────────────────────────────────────

# Run a curl request, capturing status and body separately.
# Prints an error and exits if curl cannot connect.
# Usage: hydra_request STATUS_VAR BODY_VAR <curl args...>
hydra_request() {
    local -n _status=$1
    local -n _body=$2
    shift 2

    local tmp
    tmp=$(mktemp)
    local curl_exit=0
    _status=$(curl -s -o "$tmp" -w "%{http_code}" "$@") || curl_exit=$?
    _body=$(cat "$tmp")
    rm -f "$tmp"

    if [[ $curl_exit -ne 0 ]]; then
        echo "Error: could not connect to Hydra admin API at ${ADMIN_URL}" >&2
        echo "  curl exit code: ${curl_exit}" >&2
        echo "  Is Hydra running? Check HYDRA_ADMIN_URL (currently: ${ADMIN_URL})" >&2
        exit 1
    fi
}

# ── Generate a random client secret ──────────────────────────────────────────

CLIENT_SECRET=$(openssl rand -hex 32)

# ── Build the scope string (space-separated) ──────────────────────────────────

SCOPE_STR="${SCOPES[*]}"

# ── Build the request body ────────────────────────────────────────────────────

BODY=$(jq -n \
    --arg id     "$CLIENT_ID" \
    --arg secret "$CLIENT_SECRET" \
    --arg scope  "$SCOPE_STR" \
    --arg aud    "$AUDIENCE" \
    '{
        client_id:                   $id,
        client_name:                 $id,
        client_secret:               $secret,
        grant_types:                 ["client_credentials"],
        scope:                       $scope,
        audience:                    [$aud],
        access_token_strategy:       "jwt",
        token_endpoint_auth_method:  "client_secret_post"
    }')

# ── Create or replace via the admin API ──────────────────────────────────────

STATUS="" RESP=""
hydra_request STATUS RESP \
    -X POST "${ADMIN_URL}/admin/clients" \
    -H "Content-Type: application/json" \
    -d "$BODY"

if [[ "$STATUS" == "201" ]]; then
    : # created
elif [[ "$STATUS" == "409" ]]; then
    # Client already exists — replace it so the secret is updated.
    hydra_request STATUS RESP \
        -X PUT "${ADMIN_URL}/admin/clients/${CLIENT_ID}" \
        -H "Content-Type: application/json" \
        -d "$BODY"
    if [[ "$STATUS" != "200" ]]; then
        echo "Error: PUT /admin/clients/${CLIENT_ID} returned HTTP ${STATUS}" >&2
        echo "${RESP}" >&2
        exit 1
    fi
else
    echo "Error: POST /admin/clients returned HTTP ${STATUS}" >&2
    echo "${RESP}" >&2
    exit 1
fi

# ── Print the environment variables ──────────────────────────────────────────

echo ""
echo "Client '${CLIENT_ID}' registered. Export these before running the CLI:"
echo ""
echo "export UDEX_TOKEN_URL='${TOKEN_URL}'"
echo "export UDEX_CLIENT_ID='${CLIENT_ID}'"
echo "export UDEX_CLIENT_SECRET='${CLIENT_SECRET}'"
echo ""
echo "# Then fetch a token to verify:"
echo "# udex token fetch --url '${TOKEN_URL}'"
