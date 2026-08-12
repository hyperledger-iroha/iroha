#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/build"
SOURCE_FILE="$SCRIPT_DIR/contract/api_service.ko"
BYTECODE_FILE="$OUTPUT_DIR/api-service.to"
CONTRACT_MANIFEST_FILE="$OUTPUT_DIR/api-service.contract_manifest.json"

mkdir -p "$OUTPUT_DIR"

if [[ -n "${KOTO_BIN:-}" && -x "${KOTO_BIN:-}" ]]; then
  KOTO=("$KOTO_BIN")
elif command -v koto >/dev/null 2>&1; then
  KOTO=("$(command -v koto)")
else
  if [[ -n "${IROHA_SOURCE_DIR:-}" && -f "${IROHA_SOURCE_DIR}/Cargo.toml" ]]; then
    IROHA_CARGO_MANIFEST="${IROHA_SOURCE_DIR}/Cargo.toml"
  elif [[ -n "${IROHA_MANIFEST_PATH:-}" && -f "${IROHA_MANIFEST_PATH}" ]]; then
    IROHA_CARGO_MANIFEST="$IROHA_MANIFEST_PATH"
  else
    echo "Unable to locate koto. Set KOTO_BIN or IROHA_SOURCE_DIR." >&2
    exit 1
  fi
  KOTO=(
    cargo run
    --manifest-path "$IROHA_CARGO_MANIFEST"
    -p ivm
    --bin koto
    --
  )
fi

"${KOTO[@]}" build "$SOURCE_FILE" \
  --out "$BYTECODE_FILE" \
  --manifest-out "$CONTRACT_MANIFEST_FILE" \
  --max-cycles 1000000

echo "built $BYTECODE_FILE"
