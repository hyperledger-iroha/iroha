#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
BYTECODE_FILE="$SCRIPT_DIR/build/vault-api.to"
MANIFEST_FILE="$SCRIPT_DIR/build/vault-api.contract_manifest.json"
TMP_DIR="$(mktemp -d)"

trap 'rm -rf "$TMP_DIR"' EXIT

if [[ ! -f "$BYTECODE_FILE" ]]; then
  echo "Missing $BYTECODE_FILE. Run ./build.sh first." >&2
  exit 1
fi
if [[ ! -f "$MANIFEST_FILE" ]]; then
  echo "Missing $MANIFEST_FILE. Run ./build.sh first." >&2
  exit 1
fi

if [[ -z "${KOTO_BIN:-}" || "$KOTO_BIN" != /* || ! -f "$KOTO_BIN" || ! -x "$KOTO_BIN" || -L "$KOTO_BIN" ]]; then
  echo "KOTO_BIN must name the absolute, executable, non-symlinked same-revision koto binary" >&2
  exit 1
fi
: "${KOTO_BIN_SHA256:?Set KOTO_BIN_SHA256 to the lowercase SHA-256 of KOTO_BIN}"
if [[ ! "$KOTO_BIN_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "KOTO_BIN_SHA256 must be exactly 64 lowercase hexadecimal characters" >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  KOTO_BIN_ACTUAL_SHA256="$(sha256sum "$KOTO_BIN" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  KOTO_BIN_ACTUAL_SHA256="$(shasum -a 256 "$KOTO_BIN" | awk '{print $1}')"
else
  echo "A SHA-256 tool (sha256sum or shasum) is required to qualify KOTO_BIN" >&2
  exit 1
fi
if [[ "$KOTO_BIN_ACTUAL_SHA256" != "$KOTO_BIN_SHA256" ]]; then
  echo "KOTO_BIN does not match the operator-qualified same-revision SHA-256" >&2
  exit 1
fi
KOTO=("$KOTO_BIN")

"${KOTO[@]}" build \
  "$SCRIPT_DIR/contract/vault_api.ko" \
  --out "$TMP_DIR/vault-api.to" \
  --manifest-out "$TMP_DIR/vault-api.contract_manifest.json" \
  --max-cycles 1000000

cmp -s "$BYTECODE_FILE" "$TMP_DIR/vault-api.to" || {
  echo "Compiled bytecode differs from build/vault-api.to. Re-run ./build.sh." >&2
  exit 1
}

cmp -s "$MANIFEST_FILE" "$TMP_DIR/vault-api.contract_manifest.json" || {
  echo "Compiled contract manifest differs from build/vault-api.contract_manifest.json. Re-run ./build.sh." >&2
  exit 1
}

echo "verified $BYTECODE_FILE and $MANIFEST_FILE"
