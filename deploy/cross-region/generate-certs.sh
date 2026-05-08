#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CERTS_DIR="${SCRIPT_DIR}/certs"
CA_DIR="${CERTS_DIR}/ca"

REGIONS=("us-east-1" "eu-west-1" "ap-southeast-1")
VALIDITY_DAYS=7

mkdir -p "${CERTS_DIR}" "${CA_DIR}"

echo "==> Generating CA key and certificate (valid ${VALIDITY_DAYS} days)"
openssl genpkey -algorithm Ed25519 -out "${CA_DIR}/ca.key" 2>/dev/null

cat > "${CA_DIR}/ca.cnf" <<EOF
[req]
distinguished_name = dn
prompt = no
x509_extensions = v3_ca

[dn]
CN = Aether Cross-Region CA
O = Aether
C = US

[v3_ca]
basicConstraints = critical, CA:TRUE
keyUsage = critical, digitalSignature, keyCertSign, cRLSign
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always, issuer
EOF

openssl req -new -x509 -key "${CA_DIR}/ca.key" \
  -out "${CA_DIR}/ca.crt" \
  -days "${VALIDITY_DAYS}" \
  -config "${CA_DIR}/ca.cnf" \
  -sha256

echo "==> CA certificate generated:"
openssl x509 -in "${CA_DIR}/ca.crt" -noout -subject -issuer -dates

echo ""
echo "==> Generating node certificates for each region"
for region in "${REGIONS[@]}"; do
  echo "--- Generating certificate for ${region}"

  openssl genpkey -algorithm Ed25519 -out "${CERTS_DIR}/${region}.key" 2>/dev/null

  cat > "${CERTS_DIR}/${region}.cnf" <<EOF
[req]
distinguished_name = dn
prompt = no

[dn]
CN = ${region}
O = Aether
OU = ${region}

[v3_ext]
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth, clientAuth
subjectAltName = @alt_names
authorityKeyIdentifier = keyid:always, issuer

[alt_names]
DNS.1 = ${region}
DNS.2 = aether-${region}
DNS.3 = *.${region}
EOF

  openssl req -new -key "${CERTS_DIR}/${region}.key" \
    -out "${CERTS_DIR}/${region}.csr" \
    -config "${CERTS_DIR}/${region}.cnf" \
    -sha256

  openssl x509 -req -in "${CERTS_DIR}/${region}.csr" \
    -CA "${CA_DIR}/ca.crt" -CAkey "${CA_DIR}/ca.key" \
    -CAcreateserial -out "${CERTS_DIR}/${region}.crt" \
    -days "${VALIDITY_DAYS}" \
    -extfile "${CERTS_DIR}/${region}.cnf" \
    -extensions v3_ext \
    -sha256

  rm -f "${CERTS_DIR}/${region}.csr" "${CERTS_DIR}/${region}.cnf"

  echo "    Certificate for ${region}: $(openssl x509 -in "${CERTS_DIR}/${region}.crt" -noout -serial -dates)"
done

echo ""
echo "==> Copying CA cert to shared location"
cp "${CA_DIR}/ca.crt" "${CERTS_DIR}/ca.crt"

echo ""
echo "==> Generating CRL"
cat > "${CERTS_DIR}/crl.cnf" <<EOF
[ca]
default_ca = CA_default

[CA_default]
database = ${CA_DIR}/index.txt
crlnumber = ${CA_DIR}/crlnumber
default_md = sha256
default_crl_days = ${VALIDITY_DAYS}
EOF

touch "${CA_DIR}/index.txt"
echo 01 > "${CA_DIR}/crlnumber"

openssl ca -gencrl -keyfile "${CA_DIR}/ca.key" \
  -cert "${CA_DIR}/ca.crt" \
  -out "${CERTS_DIR}/ca.crl" \
  -config "${CERTS_DIR}/crl.cnf" 2>/dev/null || {
  echo "    CRL generation failed (CA database not initialized), creating empty CRL"
  openssl ca -gencrl -keyfile "${CA_DIR}/ca.key" \
    -cert "${CA_DIR}/ca.crt" \
    -out "${CERTS_DIR}/ca.crl" \
    -config <(sed 's/database/#database/' "${CERTS_DIR}/crl.cnf") 2>/dev/null || true
}

rm -f "${CERTS_DIR}/crl.cnf"

echo ""
echo "==> Certificate generation complete"
echo "    Files in ${CERTS_DIR}/:"
ls -la "${CERTS_DIR}/" | grep -E '\.(crt|key|crl)$'
echo ""
echo "==> IMPORTANT: Distribute ${CERTS_DIR}/ to all region nodes"
echo "    Set file permissions: chmod 600 ${CERTS_DIR}/*.key"
chmod 600 "${CERTS_DIR}"/*.key 2>/dev/null || true
echo "    Set cert permissions: chmod 644 ${CERTS_DIR}"/*.crt 2>/dev/null || true
