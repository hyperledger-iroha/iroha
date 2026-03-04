#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/address_local_toolkit.sh --input PATH [options]

Automates the Local → Global address migration workflow described in
docs/source/sns/local_to_global_toolkit.md. The script wraps the iroha CLI to
emit a JSON audit report and (optionally) a converted address list that replaces
Local-domain selectors with canonical IH58 (preferred) or compressed (`sora`, second-best) strings.

Required arguments:
  --input PATH          Newline-separated addresses (IH58 (preferred)/sora (second-best)/canonical hex).

Options:
  --output-dir PATH     Directory for generated artefacts (default: artifacts/address_toolkit).
  --network-prefix NUM  IH58 network prefix for Sora (default: 42).
  --format FORMAT       Conversion format: ih58 or compressed (default: ih58).
  --append-domain       Preserve the original domain when emitting converted literals (default).
  --no-append-domain    Do not append the domain suffix to converted literals.
  --audit-only          Emit the JSON audit report without running the converter.
  --iroha-cli PATH      Override the iroha CLI binary (default: iroha).
  --allow-errors        Continue when audit/normalize hits parse errors (default: false).
  -h, --help            Show this help message and exit.
EOF
}

IROHA_CLI_BIN=${IROHA_CLI_BIN:-iroha}
OUTPUT_DIR="artifacts/address_toolkit"
NETWORK_PREFIX=42
FORMAT="ih58"
APPEND_DOMAIN=1
AUDIT_ONLY=0
ALLOW_ERRORS=0
INPUT_PATH=""

while (($#)); do
    case "$1" in
        --input)
            INPUT_PATH=${2:?--input requires a value}
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR=${2:?--output-dir requires a value}
            shift 2
            ;;
        --network-prefix)
            NETWORK_PREFIX=${2:?--network-prefix requires a value}
            shift 2
            ;;
        --format)
            FORMAT=${2:?--format requires a value}
            shift 2
            ;;
        --append-domain)
            APPEND_DOMAIN=1
            shift
            ;;
        --no-append-domain)
            APPEND_DOMAIN=0
            shift
            ;;
        --audit-only)
            AUDIT_ONLY=1
            shift
            ;;
        --iroha-cli)
            IROHA_CLI_BIN=${2:?--iroha-cli requires a value}
            shift 2
            ;;
        --allow-errors)
            ALLOW_ERRORS=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ -z "$INPUT_PATH" ]]; then
    echo "error: --input is required" >&2
    usage >&2
    exit 1
fi

if [[ "$FORMAT" != "ih58" && "$FORMAT" != "compressed" ]]; then
    echo "error: --format must be 'ih58' or 'compressed'" >&2
    exit 1
fi

mkdir -p "$OUTPUT_DIR"
AUDIT_PATH="$OUTPUT_DIR/audit.json"
CONVERT_PATH="$OUTPUT_DIR/normalized.txt"

echo "[address-local-toolkit] writing audit report to $AUDIT_PATH"
AUDIT_CMD=(
    "$IROHA_CLI_BIN" address audit
    --input "$INPUT_PATH"
    --network-prefix "$NETWORK_PREFIX"
    --format json
)
if [[ $ALLOW_ERRORS -eq 1 ]]; then
    AUDIT_CMD+=(--allow-errors)
fi
"${AUDIT_CMD[@]}" >"$AUDIT_PATH"

if [[ $AUDIT_ONLY -eq 1 ]]; then
    echo "[address-local-toolkit] audit-only mode enabled; skipping conversion step"
    echo "[address-local-toolkit] done."
    exit 0
fi

echo "[address-local-toolkit] generating normalized output at $CONVERT_PATH"
NORMALIZE_CMD=(
    "$IROHA_CLI_BIN" address normalize
    --input "$INPUT_PATH"
    --network-prefix "$NETWORK_PREFIX"
    --format "$FORMAT"
    --only-local
)
if [[ $ALLOW_ERRORS -eq 1 ]]; then
    NORMALIZE_CMD+=(--allow-errors)
fi
if [[ $APPEND_DOMAIN -eq 1 ]]; then
    NORMALIZE_CMD+=(--append-domain)
fi
"${NORMALIZE_CMD[@]}" >"$CONVERT_PATH"

echo "[address-local-toolkit] conversion completed. Review:"
echo "  Audit report     : $AUDIT_PATH"
echo "  Converted output : $CONVERT_PATH"
echo "Attach both artefacts to your Local → Global migration evidence bundle."
