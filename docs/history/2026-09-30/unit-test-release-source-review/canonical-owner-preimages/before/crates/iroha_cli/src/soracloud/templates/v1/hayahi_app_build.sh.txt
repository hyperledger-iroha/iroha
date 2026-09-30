#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/build"
SOURCE_FILE="$SCRIPT_DIR/contract/hayahi_api.ko"
BYTECODE_FILE="$OUTPUT_DIR/hayahi-app-api.to"
CONTRACT_MANIFEST_FILE="$OUTPUT_DIR/hayahi-app-api.contract_manifest.json"

mkdir -p "$OUTPUT_DIR"

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

"${KOTO[@]}" build "$SOURCE_FILE" \
  --out "$BYTECODE_FILE" \
  --manifest-out "$CONTRACT_MANIFEST_FILE" \
  --max-cycles 1000000

echo "built $BYTECODE_FILE"
