#!/bin/bash

# Traefik edge certificate generation script for local k8s (k3d) development.
#
# Generates a self-contained CA and an "edge" server certificate that Traefik
# presents to clients when it TERMINATES TLS at the ingress (L7). This is
# distinct from the pod's own certificate (projects/rust/server/tests/certs):
# after terminating the client TLS here, Traefik re-encrypts to the pod over a
# separate TLS hop.
#
# The CA generated here is what gRPC clients (SDK/CLI and the k8s integration
# tests) must trust, because they now validate THIS certificate, not the pod's.
#
# DO NOT USE THESE ANYWHERE ELSE — development/testing only!

+set -euo pipefail

SCRIPT_DIR="$(dirname "$0")"
CERT_DIR="$SCRIPT_DIR"

# Create certs directory if it doesn't exist
mkdir -p "$CERT_DIR"
cd "$CERT_DIR"

echo "Generating Traefik edge certificates in $CERT_DIR"

# Clean up existing certificates
rm -f ca.key ca.crt tls.key tls.csr tls.crt ca.srl

# Generate CA private key
echo "1. Generating edge CA private key..."
openssl genrsa -out ca.key 4096

# Generate CA certificate (self-signed)
echo "2. Generating edge CA certificate..."
openssl req -new -x509 -days 365 -key ca.key -out ca.crt -subj "/C=US/ST=Test/L=Test/O=Udex Traefik Edge CA/OU=Testing/CN=Udex Traefik Edge CA"

# Generate edge server private key
echo "3. Generating edge server private key..."
openssl genrsa -out tls.key 4096

# Generate edge server certificate signing request (CSR)
echo "4. Generating edge server CSR..."
openssl req -new -key tls.key -out tls.csr -subj "/C=US/ST=Test/L=Test/O=Udex Traefik Edge/OU=Testing/CN=host.docker.internal"

# Generate edge server certificate signed by the edge CA.
# SANs must cover every host clients use to reach the ingress:
#   host.docker.internal — devcontainer default (K8S_SERVER_URL)
#   localhost / 127.0.0.1 — CI and bare-metal
echo "5. Generating edge server certificate..."
openssl x509 -req -in tls.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out tls.crt -days 365 -extensions v3_req -extfile <(
cat <<EOF
[v3_req]
keyUsage = keyEncipherment, dataEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = host.docker.internal
IP.1 = 127.0.0.1
IP.2 = ::1
EOF
)

# Set appropriate permissions
chmod 600 *.key
chmod 644 *.crt *.csr

# Note: we do NOT install ca.crt into the system trust store. The Rust k8s
# integration tests load it explicitly via the SDK's ca_cert_pem_bytes, so
# system-level trust is never needed.

echo ""
echo "Edge certificate generation complete!"
echo "Generated files:"
echo "  ca.key   - edge CA private key"
echo "  ca.crt   - edge CA certificate (clients must trust this)"
echo "  tls.key  - edge server private key (mounted by Traefik)"
echo "  tls.csr  - edge server certificate signing request"
echo "  tls.crt  - edge server certificate (signed by edge CA)"
echo ""
echo "Note: These certificates are for local development only — never use in production!"

