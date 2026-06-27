#!/bin/bash

# OTLP TLS certificate generation for the local observability stack (ST0026).
#
# Generates a self-contained CA and a server certificate that the OpenTelemetry
# Collector presents on its OTLP endpoints (gRPC 4317 / HTTP 4318). The Udex app
# (server, CLI, SDK-in-host) trusts this CA via its `observability.otlp_ca`
# setting, consistent with the project's TLS-everywhere principle.
#
# This is independent of the pod cert (projects/rust/server/tests/certs) and the
# Traefik edge cert (projects/k8s/traefik/certs).
#
# DO NOT USE THESE ANYWHERE ELSE - development/testing only!

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CERT_DIR="$SCRIPT_DIR"

mkdir -p "$CERT_DIR"
cd "$CERT_DIR"

echo "Generating OTLP collector certificates in $CERT_DIR"

# Clean up existing certificates
rm -f ca.key ca.crt collector.key collector.csr collector.crt ca.srl

echo "1. Generating OTLP CA private key..."
openssl genrsa -out ca.key 4096

echo "2. Generating OTLP CA certificate..."
openssl req -new -x509 -days 365 -key ca.key -out ca.crt \
  -subj "/C=US/ST=Test/L=Test/O=Udex OTLP CA/OU=Testing/CN=Udex OTLP CA"

echo "3. Generating collector server private key..."
openssl genrsa -out collector.key 4096

echo "4. Generating collector server CSR..."
openssl req -new -key collector.key -out collector.csr \
  -subj "/C=US/ST=Test/L=Test/O=Udex OTLP/OU=Testing/CN=otel-collector"

# SANs must cover every host the app uses to reach the collector:
#   otel-collector        - compose/devcontainer service name (in-network)
#   localhost / 127.0.0.1 - host apps via the published port
#   host.docker.internal  - devcontainer/host bridge
#   host.k3d.internal     - k3d pods reaching the host-published collector port
echo "5. Generating collector server certificate..."
openssl x509 -req -in collector.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out collector.crt -days 365 -extensions v3_req -extfile <(
cat <<EOF
[v3_req]
keyUsage = keyEncipherment, dataEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = otel-collector
DNS.2 = localhost
DNS.3 = host.docker.internal
DNS.4 = host.k3d.internal
IP.1 = 127.0.0.1
IP.2 = ::1
EOF
)

chmod 600 ./*.key
chmod 644 ./*.crt ./*.csr

echo ""
echo "OTLP certificate generation complete!"
echo "Generated files:"
echo "  ca.key        - OTLP CA private key"
echo "  ca.crt        - OTLP CA certificate (the app must trust this)"
echo "  collector.key - collector server private key (mounted by the collector)"
echo "  collector.csr - collector server certificate signing request"
echo "  collector.crt - collector server certificate (signed by the OTLP CA)"
echo ""
echo "Note: These certificates are for local development only - never use in production!"
