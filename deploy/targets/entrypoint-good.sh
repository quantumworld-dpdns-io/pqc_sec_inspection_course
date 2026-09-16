#!/bin/sh
# The healthy lab target.
#
# It serves two certificates so both classical and post-quantum signature probes have
# something to negotiate: an ECDSA P-256 leaf and an ML-DSA-65 leaf (OpenSSL 3.5 issues and
# serves both natively). Without the second certificate every `-sigalgs mldsa65` probe would
# fail on "no shared signature algorithm" and look like a PQC problem when it is a fixture
# problem.
#
#   :11000 — 正常 / A server: full PQC group list, post-quantum groups first
#   :11001 — B server: the preference order the KEM priority probe recovers
set -eu

CERT_DIR=/tmp/certs
mkdir -p "$CERT_DIR"
cd "$CERT_DIR"

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes \
  -keyout ec.key -out ec.crt -days 365 -subj "/CN=tls-good.lab" >/dev/null 2>&1
openssl req -x509 -newkey mldsa65 -nodes \
  -keyout mldsa.key -out mldsa.crt -days 365 -subj "/CN=tls-good.lab" >/dev/null 2>&1

GOOD_GROUPS="${GOOD_GROUPS:-X25519MLKEM768:SecP256r1MLKEM768:SecP384r1MLKEM1024:MLKEM1024:MLKEM768:MLKEM512:x25519:P-256:P-384}"
B_GROUPS="${B_GROUPS:-MLKEM1024:MLKEM768:X25519MLKEM768:x25519:P-256:P-384}"

openssl s_server -accept 11001 -www -quiet \
  -cert ec.crt -key ec.key -dcert mldsa.crt -dkey mldsa.key \
  -groups "$B_GROUPS" &

exec openssl s_server -accept 11000 -www \
  -cert ec.crt -key ec.key -dcert mldsa.crt -dkey mldsa.key \
  -groups "$GOOD_GROUPS"
