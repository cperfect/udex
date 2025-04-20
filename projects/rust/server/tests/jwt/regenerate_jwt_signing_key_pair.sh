#!/bin/bash

# Script to (re)generate ECDSA key pair for JWT signing
# This generates ES256 (ECDSA P-256 curve) keys suitable for JWT signing

set -e  # Exit on any error

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRIVATE_KEY_FILE="${SCRIPT_DIR}/signing_private_key.pem"
PUBLIC_KEY_FILE="${SCRIPT_DIR}/signing_public_key.pem"
BAD_PRIVATE_KEY_FILE="${SCRIPT_DIR}/bad_signing_private_key.pem"
BAD_PUBLIC_KEY_FILE="${SCRIPT_DIR}/bad_signing_public_key.pem"

echo "🔑 Generating ECDSA key pairs for JWT signing..."

# Remove existing good keys if they exist
if [[ -f "${PRIVATE_KEY_FILE}" ]]; then
    echo "⚠️  Removing existing private key: ${PRIVATE_KEY_FILE}"
    rm "${PRIVATE_KEY_FILE}"
fi

if [[ -f "${PUBLIC_KEY_FILE}" ]]; then
    echo "⚠️  Removing existing public key: ${PUBLIC_KEY_FILE}"
    rm "${PUBLIC_KEY_FILE}"
fi

# Remove existing bad keys if they exist
if [[ -f "${BAD_PRIVATE_KEY_FILE}" ]]; then
    echo "⚠️  Removing existing bad private key: ${BAD_PRIVATE_KEY_FILE}"
    rm "${BAD_PRIVATE_KEY_FILE}"
fi

if [[ -f "${BAD_PUBLIC_KEY_FILE}" ]]; then
    echo "⚠️  Removing existing bad public key: ${BAD_PUBLIC_KEY_FILE}"
    rm "${BAD_PUBLIC_KEY_FILE}"
fi

echo "📝 Generating primary ECDSA private key (P-256 curve)..."
# Generate ECDSA private key using P-256 curve (suitable for ES256 JWT algorithm) - must be in pkscs8 format for jsonwebtoken crate
openssl ecparam -genkey -name prime256v1 -noout | openssl pkcs8 -topk8 -nocrypt -out "${PRIVATE_KEY_FILE}"

echo "📝 Extracting primary public key from private key..."
# Extract the corresponding public key
openssl ec -in "${PRIVATE_KEY_FILE}" -pubout -out "${PUBLIC_KEY_FILE}"

echo "📝 Generating bad ECDSA private key (P-256 curve) for testing..."
# Generate a second ECDSA private key (different from the first) for testing invalid signatures
openssl ecparam -genkey -name prime256v1 -noout | openssl pkcs8 -topk8 -nocrypt -out "${BAD_PRIVATE_KEY_FILE}"

echo "📝 Extracting bad public key from bad private key..."
# Extract the corresponding public key
openssl ec -in "${BAD_PRIVATE_KEY_FILE}" -pubout -out "${BAD_PUBLIC_KEY_FILE}"

# Set appropriate permissions (readable by owner only for private keys)
chmod 600 "${PRIVATE_KEY_FILE}"
chmod 644 "${PUBLIC_KEY_FILE}"
chmod 600 "${BAD_PRIVATE_KEY_FILE}"
chmod 644 "${BAD_PUBLIC_KEY_FILE}"

echo "✅ ECDSA key pairs generated successfully!"
echo "   Primary private key: ${PRIVATE_KEY_FILE}"
echo "   Primary public key:  ${PUBLIC_KEY_FILE}"
echo "   Bad private key:     ${BAD_PRIVATE_KEY_FILE}"
echo "   Bad public key:      ${BAD_PUBLIC_KEY_FILE}"
echo ""
echo "🔍 Key information:"
echo "   Algorithm: ECDSA"
echo "   Curve: P-256 (prime256v1)"
echo "   JWT Algorithm: ES256"
echo ""
echo "📋 To use these keys in your JWT implementation:"
echo "   - Use the primary private key for signing valid JWT tokens"
echo "   - Use the primary public key for verifying JWT tokens"
echo "   - Use the bad private key for testing invalid signatures (tokens signed with this will fail verification)"
echo "   - Set JWT algorithm to 'ES256' in your configuration"
