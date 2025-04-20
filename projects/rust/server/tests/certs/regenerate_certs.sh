#!/bin/bash

# Certificate generation script for testing
# Generates fake CA, server key, CSR, and certificate for development/testing purposes
# DO NOT USE THESE ANYWHERE ELSE!

set -e

SCRIPT_DIR="$(dirname "$0")"
CERT_DIR="$SCRIPT_DIR"

# Create certs directory if it doesn't exist
mkdir -p "$CERT_DIR"
cd "$CERT_DIR"

echo "Generating test certificates in $CERT_DIR"

# Clean up existing certificates
rm -f ca.key ca.crt server.key server.csr server.crt

# Generate CA private key
echo "1. Generating CA private key..."
openssl genrsa -out ca.key 4096

# Generate CA certificate (self-signed)
echo "2. Generating CA certificate..."
openssl req -new -x509 -days 365 -key ca.key -out ca.crt -subj "/C=US/ST=Test/L=Test/O=Udex Test CA/OU=Testing/CN=Udex Test CA"

# Generate server private key
echo "3. Generating server private key..."
openssl genrsa -out server.key 4096

# Generate server certificate signing request (CSR)
echo "4. Generating server CSR..."
openssl req -new -key server.key -out server.csr -subj "/C=US/ST=Test/L=Test/O=Udex Server/OU=Testing/CN=localhost"

# Generate server certificate signed by CA
echo "5. Generating server certificate..."
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 365 -extensions v3_req -extfile <(
cat <<EOF
[v3_req]
keyUsage = keyEncipherment, dataEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = 127.0.0.1
IP.1 = 127.0.0.1
IP.2 = ::1
EOF
)

# Set appropriate permissions
chmod 600 *.key
chmod 644 *.crt *.csr

# Add CA to system trusted certificates (requires sudo)
echo "6. Adding CA to system trusted certificates..."
if [ -f "/etc/ca-certificates.conf" ] && [ -d "/usr/local/share/ca-certificates" ]; then
    # Debian/Ubuntu system
    CA_FILE="/usr/local/share/ca-certificates/udex-test-ca.crt"
    if sudo cp ca.crt "$CA_FILE" 2>/dev/null; then
        sudo update-ca-certificates
        echo "   ✓ CA added to system trust store (Debian/Ubuntu)"
    else
        echo "   ⚠ Failed to add CA to system trust store (insufficient permissions or not Debian/Ubuntu)"
        echo "   Manual command: sudo cp ca.crt /usr/local/share/ca-certificates/udex-test-ca.crt && sudo update-ca-certificates"
    fi
elif [ -f "/etc/pki/tls/certs/ca-bundle.crt" ]; then
    # RHEL/CentOS system
    CA_FILE="/etc/pki/ca-trust/source/anchors/udex-test-ca.crt"
    if sudo cp ca.crt "$CA_FILE" 2>/dev/null; then
        sudo update-ca-trust
        echo "   ✓ CA added to system trust store (RHEL/CentOS)"
    else
        echo "   ⚠ Failed to add CA to system trust store (insufficient permissions or not RHEL/CentOS)"
        echo "   Manual command: sudo cp ca.crt /etc/pki/ca-trust/source/anchors/udex-test-ca.crt && sudo update-ca-trust"
    fi
else
    echo "   ⚠ Unknown Linux distribution - could not automatically add CA to system trust store"
    echo "   You may need to manually add ca.crt to your system's trusted certificates"
fi

echo ""
echo "Certificate generation complete!"
echo "Generated files:"
echo "  ca.key      - CA private key"
echo "  ca.crt      - CA certificate"
echo "  server.key  - Server private key"
echo "  server.csr  - Server certificate signing request"
echo "  server.crt  - Server certificate (signed by CA)"
echo ""
echo "Testing commands:"
echo "  curl --cacert ca.crt https://localhost:port/    # Using explicit CA cert"
echo "  curl https://localhost:port/                     # Using system trust store (if CA was installed)"
echo ""
echo "To remove the CA from system trust store:"
echo "  sudo rm /usr/local/share/ca-certificates/udex-test-ca.crt && sudo update-ca-certificates  # Debian/Ubuntu"
echo "  sudo rm /etc/pki/ca-trust/source/anchors/udex-test-ca.crt && sudo update-ca-trust         # RHEL/CentOS"
echo ""
echo "Note: These certificates are for testing only and should not be used in production!"