#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
BYTECODE_FILE="$SCRIPT_DIR/build/api-service.to"
MANIFEST_FILE="$SCRIPT_DIR/build/api-service.contract_manifest.json"
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

"${KOTO[@]}" build \
  "$SCRIPT_DIR/contract/api_service.ko" \
  --out "$TMP_DIR/api-service.to" \
  --manifest-out "$TMP_DIR/api-service.contract_manifest.json" \
  --max-cycles 1000000

cmp -s "$BYTECODE_FILE" "$TMP_DIR/api-service.to" || {
  echo "Compiled bytecode differs from build/api-service.to. Re-run ./build.sh." >&2
  exit 1
}

cmp -s "$MANIFEST_FILE" "$TMP_DIR/api-service.contract_manifest.json" || {
  echo "Compiled contract manifest differs from build/api-service.contract_manifest.json. Re-run ./build.sh." >&2
  exit 1
}

echo "verified $BYTECODE_FILE and $MANIFEST_FILE"
