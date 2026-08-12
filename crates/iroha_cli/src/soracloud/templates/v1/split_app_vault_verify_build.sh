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

if [[ -n "${KOTO_BIN:-}" ]]; then
  KOTO=("${KOTO_BIN}")
elif command -v koto >/dev/null 2>&1; then
  KOTO=("$(command -v koto)")
elif [[ -n "${IROHA_MANIFEST_PATH:-}" ]]; then
  KOTO=(cargo run --manifest-path "$IROHA_MANIFEST_PATH" -p ivm --bin koto --)
else
  echo "Unable to locate koto. Set KOTO_BIN or IROHA_MANIFEST_PATH." >&2
  exit 1
fi

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
