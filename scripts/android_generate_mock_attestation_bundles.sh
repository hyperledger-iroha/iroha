#!/usr/bin/env bash
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

ROOT_KEY="artifacts/android/attestation/.tmp/mock_root.key"
ROOT_CERT="artifacts/android/attestation/trust_root_mock.pem"
TMP_DIR="artifacts/android/attestation/.tmp"

if [[ ! -f "$ROOT_KEY" || ! -f "$ROOT_CERT" ]]; then
  echo "missing root key/cert" >&2
  exit 1
fi

mkdir -p "$TMP_DIR"

create_bundle() {
  local device=$1
  local date=$2
  local alias=$3
  local vendor=$4

  local dir="artifacts/android/attestation/${device}/${date}"
  mkdir -p "$dir"

  local challenge lowercase_challenge unique lowercase_unique derhex
  challenge=$(openssl rand -hex 32 | tr '[:lower:]' '[:upper:]')
  lowercase_challenge=$(echo "$challenge" | tr '[:upper:]' '[:lower:]')
  unique=$(openssl rand -hex 16 | tr '[:lower:]' '[:upper:]')
  lowercase_unique=$(echo "$unique" | tr '[:upper:]' '[:lower:]')
  derhex=$(scripts/android_mock_attestation_der.py "$lowercase_challenge" "$lowercase_unique")

  local key="$TMP_DIR/${device}.key"
  local csr="$TMP_DIR/${device}.csr"
  local crt="$TMP_DIR/${device}.crt"
  local ext="$TMP_DIR/${device}.ext"

  openssl req -new -newkey rsa:2048 -nodes -keyout "$key" -out "$csr" -subj "/CN=${device}" >/dev/null 2>&1

  cat > "$ext" <<EOT
basicConstraints=CA:FALSE
keyUsage = digitalSignature
extendedKeyUsage = clientAuth
1.3.6.1.4.1.11129.2.1.17=DER:${derhex}
EOT

  openssl x509 -req -in "$csr" -CA "$ROOT_CERT" -CAkey "$ROOT_KEY" -CAcreateserial -out "$crt" -days 825 -extfile "$ext" >/dev/null 2>&1

  cat "$crt" "$ROOT_CERT" > "$dir/chain.pem"
  printf "%s\n" "$alias" > "$dir/alias.txt"
  printf "%s\n" "$challenge" > "$dir/challenge.hex"
  printf '{\n  "alias": "%s",\n  "attestation_security_level": "STRONGBOX",\n  "keymaster_security_level": "STRONGBOX",\n  "strongbox_attestation": true,\n  "challenge_hex": "%s",\n  "chain_length": 2\n}\n' "$alias" "$challenge" > "$dir/result.json"
  cp "$ROOT_CERT" "$dir/trust_root_${vendor}.pem"
  printf "Mock bundle generated on %sZ with challenge %s and unique ID %s.\n" "$(date -u +"%Y-%m-%dT%H:%M:%S")" "$challenge" "$unique" > "$dir/notes.md"

  rm -f "$key" "$csr" "$crt" "$ext"
}

create_bundle "pixel6-strongbox-a" "2026-02-12" "prod-strongbox-pixel6" "google"
create_bundle "pixel6a-strongbox-a" "2026-02-18" "prod-strongbox-pixel6a" "google"
create_bundle "pixel7-strongbox-a" "2026-02-12" "prod-strongbox-pixel7" "google"
create_bundle "pixel8-strongbox-a" "2026-02-14" "prod-strongbox-pixel8" "google"
create_bundle "pixel8a-strongbox-a" "2026-02-20" "prod-strongbox-pixel8a" "google"
create_bundle "pixel8pro-strongbox-a" "2026-02-13" "prod-strongbox-pixel8pro" "google"
create_bundle "pixelfold-strongbox-a" "2026-02-20" "prod-strongbox-pixelfold" "google"
create_bundle "pixeltablet-strongbox-a" "2026-02-21" "prod-strongbox-pixeltablet" "google"
create_bundle "s23-strongbox-a" "2026-02-12" "prod-strongbox-s23" "samsung"
create_bundle "s24-strongbox-a" "2026-02-13" "prod-strongbox-s24" "samsung"
