#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/build"
SOURCE_FILE="$SCRIPT_DIR/contract/vault_api.ko"
BYTECODE_FILE="$OUTPUT_DIR/vault-api.to"
CONTRACT_MANIFEST_FILE="$OUTPUT_DIR/vault-api.contract_manifest.json"

mkdir -p "$OUTPUT_DIR"

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
  "$SOURCE_FILE" \
  --out "$BYTECODE_FILE" \
  --manifest-out "$CONTRACT_MANIFEST_FILE" \
  --max-cycles 1000000

echo "built $BYTECODE_FILE"
