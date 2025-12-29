#!/usr/bin/env bash
set -euo pipefail

# Helper script for deriving demo keypairs using the Rust toolchain.
#
# Usage: ./generate-keys.sh alice
#
# Requires: cargo, iroha_crypto CLI utilities

show_help() {
  cat <<'USAGE'
Usage: generate-keys.sh <account-name>

Environment variables:
  OUTPUT_DIR   Directory where generated keys are written (defaults to ../Configs)
USAGE
}

if [[ $# -ne 1 ]]; then
  show_help >&2
  exit 1
fi

case "$1" in
  -h|--help)
    show_help
    exit 0
    ;;
  *)
    ACCOUNT_NAME="$1"
    ;;
fi

OUTPUT_DIR="${OUTPUT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/Configs}"

mkdir -p "$OUTPUT_DIR"

cargo run -p iroha_crypto --bin iroha_keygen -- "$ACCOUNT_NAME" > "$OUTPUT_DIR/${ACCOUNT_NAME}.key"

echo "Generated key written to $OUTPUT_DIR/${ACCOUNT_NAME}.key"
