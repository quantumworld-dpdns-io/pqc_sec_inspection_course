#!/bin/sh
# The 漏洞 server: a legacy-only target.
#
# It serves an ECDSA P-256 certificate over TLS 1.2 with classical groups only, so classical
# probes still succeed and every post-quantum probe fails. That contrast is the lesson — a
# target that fails *everything* would just look broken.
set -eu

CERT_DIR=/tmp/certs
mkdir -p "$CERT_DIR"
cd "$CERT_DIR"

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes \
  -keyout ec.key -out ec.crt -days 365 -subj "/CN=tls-vuln.lab" >/dev/null 2>&1

exec openssl s_server -accept 8888 -www \
  -cert ec.crt -key ec.key \
  -tls1_2 -groups "${VULN_GROUPS:-P-256:x25519}"
