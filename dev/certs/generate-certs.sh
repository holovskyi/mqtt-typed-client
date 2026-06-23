#!/usr/bin/env bash
#
# Generate throwaway self-signed TLS certificates for the LOCAL dev EMQX broker
# (localhost only). These are NOT secrets and are gitignored — never commit them.
# Run once after cloning (and any time you want fresh certs):
#
#     ./dev/certs/generate-certs.sh
#
# Produces, in this directory:
#   ca.pem      - local "Test CA" certificate (loaded by examples/004_hello_world_tls.rs)
#   cacert.pem  - copy of ca.pem (legacy filename kept for compatibility)
#   cert.pem    - server certificate for CN=localhost (used by docker-compose)
#   key.pem     - server private key                 (used by docker-compose)
#
set -euo pipefail
cd "$(dirname "$0")"

# On Git Bash / MSYS, a leading-slash `-subj "/C=US/..."` is mangled into a
# Windows path. This disables that conversion; it is harmless on Linux/macOS.
export MSYS_NO_PATHCONV=1

DAYS=825  # < 825 keeps some TLS stacks happy for leaf certs

echo "Generating local dev CA..."
openssl req -x509 -newkey rsa:2048 -nodes \
	-keyout ca-key.pem -out ca.pem -days "$DAYS" \
	-subj "/C=US/ST=CA/L=Test/O=Test/CN=Test CA"

echo "Generating server key + CSR (CN=localhost)..."
openssl req -newkey rsa:2048 -nodes \
	-keyout key.pem -out server.csr \
	-subj "/C=US/ST=CA/L=Test/O=Test/CN=localhost"

echo "Signing server certificate with the CA (SAN: localhost, 127.0.0.1)..."
# A real file with a RELATIVE name in the cwd — not process substitution and not
# an absolute path: a native (non-MSYS) openssl on Git Bash cannot read the
# /dev/fd/NN of `<(...)` nor an absolute /tmp/... MSYS path. A relative name is
# resolved correctly by both.
san_ext="san_ext.cnf"
printf "subjectAltName=DNS:localhost,IP:127.0.0.1\n" > "$san_ext"
openssl x509 -req -in server.csr \
	-CA ca.pem -CAkey ca-key.pem -CAcreateserial \
	-out cert.pem -days "$DAYS" -extfile "$san_ext"
rm -f "$san_ext"

cp ca.pem cacert.pem

# Drop intermediate / CA-private material — only the four files above are needed
# locally, and we do not want the CA private key lying around.
rm -f server.csr ca-key.pem san_ext.cnf ./*.srl

echo "Done. Created: ca.pem, cacert.pem, cert.pem, key.pem"
