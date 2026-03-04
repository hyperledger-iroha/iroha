#!/usr/bin/env bash
set -euo pipefail

# Generates the Android vs Rust telemetry schema diff artefact used in AND7 docs.
# Supports both commit-based and config-based workflows so runbooks can diff live
# manifests without pulling history.

usage() {
  cat <<'EOF'
Usage:
  scripts/telemetry/run_schema_diff.sh --android-config <file> --rust-config <file> [--out <path>]
  scripts/telemetry/run_schema_diff.sh --android-commit <sha> --rust-commit <sha> [--out <path>]
  scripts/telemetry/run_schema_diff.sh --android-config <file> --rust-config <file> --policy-out <path>
  scripts/telemetry/run_schema_diff.sh --android-config <file> --rust-config <file> --markdown-out <path>

Options:
  --android-config PATH   Path to the Android telemetry config JSON to diff.
  --rust-config PATH      Path to the Rust telemetry config JSON to diff.
  --android-commit SHA    Android git commit to diff (commit mode).
  --rust-commit SHA       Rust git commit to diff (commit mode).
  --out PATH              Explicit output path. Defaults to docs/.../android_vs_rust-<date>.json.
  --policy-out PATH       Optional policy summary output (trimmed JSON for governance bundles).
  --markdown-out PATH     Optional Markdown summary output for readiness packets.
  --metrics-out PATH      Optional Prometheus textfile path for run metrics.
  --textfile-dir PATH     Mirror metrics to a node_exporter textfile directory (default from ANDROID_SCHEMA_DIFF_TEXTFILE_DIR).
  -h, --help              Show this message.

Examples:
  scripts/telemetry/run_schema_diff.sh main android-and7-branch
  scripts/telemetry/run_schema_diff.sh \
    --android-config configs/android_telemetry.json \
    --rust-config configs/rust_telemetry.json \
    --policy-out docs/source/sdk/android/readiness/schema_diffs/$(date +%Y%m%d)-policy.json \
    --out docs/source/sdk/android/readiness/schema_diffs/$(date +%Y%m%d).json
EOF
}

ANDROID_COMMIT=""
RUST_COMMIT=""
ANDROID_CONFIG=""
RUST_CONFIG=""
OUT_FILE=""
POLICY_OUT=""
METRICS_OUT=""
TEXTFILE_DIR=""
MARKDOWN_OUT=""
DEFAULT_OUT_DIR="docs/source/sdk/android/readiness/schema_diffs"
DEFAULT_METRICS_PATH="artifacts/android/telemetry/schema_diff.prom"
TIMESTAMP="$(date -u +%Y%m%d)"

POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --android-config)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--android-config requires a path argument" >&2
        exit 1
      fi
      ANDROID_CONFIG="$1"
      ;;
    --rust-config)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--rust-config requires a path argument" >&2
        exit 1
      fi
      RUST_CONFIG="$1"
      ;;
    --android-commit)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--android-commit requires a commit SHA" >&2
        exit 1
      fi
      ANDROID_COMMIT="$1"
      ;;
    --rust-commit)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--rust-commit requires a commit SHA" >&2
        exit 1
      fi
      RUST_COMMIT="$1"
      ;;
    --out|-o)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--out requires a path argument" >&2
        exit 1
      fi
      OUT_FILE="$1"
      ;;
    --policy-out)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--policy-out requires a path argument" >&2
        exit 1
      fi
      POLICY_OUT="$1"
      ;;
    --metrics-out)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--metrics-out requires a path argument" >&2
        exit 1
      fi
      METRICS_OUT="$1"
      ;;
    --markdown-out)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--markdown-out requires a path argument" >&2
        exit 1
      fi
      MARKDOWN_OUT="$1"
      ;;
    --textfile-dir)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--textfile-dir requires a path argument" >&2
        exit 1
      fi
      TEXTFILE_DIR="$1"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      POSITIONAL+=("$@")
      break
      ;;
    -*)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
    *)
      POSITIONAL+=("$1")
      ;;
  esac
  shift || true
done

if [[ ${#POSITIONAL[@]} -gt 0 ]]; then
  echo "Unexpected positional arguments: ${POSITIONAL[*]}" >&2
  usage >&2
  exit 1
fi

if [[ -n "${ANDROID_CONFIG}" && -z "${RUST_CONFIG}" ]] || [[ -z "${ANDROID_CONFIG}" && -n "${RUST_CONFIG}" ]]; then
  echo "Both --android-config and --rust-config are required when using config mode." >&2
  exit 1
fi

if [[ -n "${ANDROID_COMMIT}" && -z "${RUST_COMMIT}" ]] || [[ -z "${ANDROID_COMMIT}" && -n "${RUST_COMMIT}" ]]; then
  echo "Both --android-commit and --rust-commit are required when using commit mode." >&2
  exit 1
fi

if [[ -z "${ANDROID_CONFIG}" && -z "${ANDROID_COMMIT}" ]]; then
  echo "Specify either config paths or commit SHAs. See --help for details." >&2
  exit 1
fi

if [[ -n "${ANDROID_CONFIG}" && -n "${ANDROID_COMMIT}" ]]; then
  echo "Do not mix config and commit modes in the same invocation." >&2
  exit 1
fi

if [[ -n "${ANDROID_CONFIG}" && ! -f "${ANDROID_CONFIG}" ]]; then
  echo "Android config not found: ${ANDROID_CONFIG}" >&2
  exit 1
fi
if [[ -n "${RUST_CONFIG}" && ! -f "${RUST_CONFIG}" ]]; then
  echo "Rust config not found: ${RUST_CONFIG}" >&2
  exit 1
fi

if [[ -z "${OUT_FILE}" ]]; then
  mkdir -p "${DEFAULT_OUT_DIR}"
  OUT_FILE="${DEFAULT_OUT_DIR}/android_vs_rust-${TIMESTAMP}.json"
else
  mkdir -p "$(dirname "${OUT_FILE}")"
fi
if [[ -z "${METRICS_OUT}" ]]; then
  METRICS_OUT="${DEFAULT_METRICS_PATH}"
fi
if [[ -n "${POLICY_OUT}" ]]; then
  mkdir -p "$(dirname "${POLICY_OUT}")"
fi
if [[ -n "${MARKDOWN_OUT}" ]]; then
  mkdir -p "$(dirname "${MARKDOWN_OUT}")"
fi
mkdir -p "$(dirname "${METRICS_OUT}")"

echo "[telemetry-schema] writing schema diff to ${OUT_FILE}" >&2
if [[ -n "${POLICY_OUT}" ]]; then
  echo "[telemetry-schema] writing policy summary to ${POLICY_OUT}" >&2
fi
if [[ -n "${METRICS_OUT}" ]]; then
  echo "[telemetry-schema] writing metrics to ${METRICS_OUT}" >&2
fi
if [[ -n "${MARKDOWN_OUT}" ]]; then
  echo "[telemetry-schema] writing markdown summary to ${MARKDOWN_OUT}" >&2
fi

cmd=(cargo run -p telemetry-schema-diff -- --format json)
if [[ -n "${ANDROID_CONFIG}" ]]; then
  cmd+=("--android-config" "${ANDROID_CONFIG}" "--rust-config" "${RUST_CONFIG}")
else
  cmd+=("--android-commit" "${ANDROID_COMMIT}" "--rust-commit" "${RUST_COMMIT}")
fi
if [[ -n "${POLICY_OUT}" ]]; then
  cmd+=("--policy-out" "${POLICY_OUT}")
fi
if [[ -n "${METRICS_OUT}" ]]; then
  cmd+=("--metrics-out" "${METRICS_OUT}")
fi
if [[ -n "${MARKDOWN_OUT}" ]]; then
  cmd+=("--markdown-out" "${MARKDOWN_OUT}")
fi

"${cmd[@]}" > "${OUT_FILE}"

echo "Schema diff written to ${OUT_FILE}" >&2

textfile_target="${TEXTFILE_DIR:-${ANDROID_SCHEMA_DIFF_TEXTFILE_DIR:-}}"
if [[ -n "${textfile_target}" ]]; then
  mkdir -p "${textfile_target}"
  tmp_file="$(mktemp "${textfile_target}/android_schema_diff.prom.XXXXXX")"
  cp "${METRICS_OUT}" "${tmp_file}"
  mv "${tmp_file}" "${textfile_target}/android_schema_diff.prom"
  echo "[telemetry-schema] mirrored metrics to ${textfile_target}/android_schema_diff.prom" >&2
fi
